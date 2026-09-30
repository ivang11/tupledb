use crate::connections::{Connection, DatabaseEngine, DatabaseSettings, SshAuth};
use crate::database::registry::{self, ConnectOptions};
use crate::ssh::SshTunnel;
use serde::Serialize;
use std::future::Future;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::net::{lookup_host, TcpStream};
use tokio::time::{timeout_at, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TestField {
    Host,
    Port,
    User,
    Password,
    Database,
    Tls,
    SshHost,
    SshPort,
    SshUser,
    SshPassword,
    SshKey,
    SshPassphrase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    Checking,
    Success,
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConnectionTestProgress {
    pub fields: Vec<TestField>,
    pub status: TestStatus,
    pub message: String,
}

pub type ProgressReporter = Arc<dyn Fn(ConnectionTestProgress) + Send + Sync>;

fn report(reporter: &ProgressReporter, fields: &[TestField], status: TestStatus, message: &str) {
    reporter(ConnectionTestProgress {
        fields: fields.to_vec(),
        status,
        message: message.into(),
    });
}

async fn step<T>(
    reporter: &ProgressReporter,
    deadline: Instant,
    fields: &[TestField],
    checking: &str,
    success: &str,
    work: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    report(reporter, fields, TestStatus::Checking, checking);
    let result = timeout_at(deadline, work)
        .await
        .unwrap_or_else(|_| Err(format!("Connection test timed out: {checking}")));
    match &result {
        Ok(_) => report(reporter, fields, TestStatus::Success, success),
        Err(error) => report(reporter, fields, TestStatus::Error, error),
    }
    result
}

async fn check_endpoint(
    reporter: &ProgressReporter,
    deadline: Instant,
    host: &str,
    port: u16,
    ssh: bool,
) -> Result<(), String> {
    let (host_field, port_field) = if ssh {
        (TestField::SshHost, TestField::SshPort)
    } else {
        (TestField::Host, TestField::Port)
    };
    let addresses = step(
        reporter,
        deadline,
        &[host_field],
        if ssh {
            "Resolving SSH host…"
        } else {
            "Resolving database host…"
        },
        "Host resolved",
        async {
            let addresses: Vec<_> = lookup_host((host, port))
                .await
                .map_err(|error| format!("Could not resolve host: {error}"))?
                .collect();
            if addresses.is_empty() {
                return Err("Host did not resolve to an address".into());
            }
            Ok(addresses)
        },
    )
    .await?;
    step(
        reporter,
        deadline,
        &[port_field],
        if ssh {
            "Connecting to SSH server…"
        } else {
            "Connecting to database server…"
        },
        "Server port reachable",
        async {
            // A TCP connection proves reachability, not database authentication.
            // Do not mark credentials as valid until the adapter accepts them.
            TcpStream::connect(addresses.as_slice())
                .await
                .map_err(|error| format!("Could not reach server port: {error}"))?;
            Ok(())
        },
    )
    .await
}

/// Diagnostic sessions never enter AppState and never replace an open workspace.
/// Every stage shares the configured timeout; progress contains no credentials.
pub async fn test(connection: &Connection, reporter: ProgressReporter) -> Result<String, String> {
    registry::ensure_available(connection.database.engine())?;
    let (host, port) = connection
        .database
        .network_endpoint()
        .ok_or("A network endpoint is required")?;
    let timeout_secs = connection.timeout_secs.unwrap_or(30).max(1);
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    if let DatabaseSettings::PostgreSql(settings) = &connection.database {
        // Reuse the adapter's configuration checks before doing network work.
        // This catches missing CA/user settings and unsupported SSH TLS modes.
        let options = ConnectOptions {
            read_only: !connection.allow_writes,
            endpoint: Some((host, port)),
            timeout_secs,
            tunneled: connection.ssh.is_some(),
            on_connected: None,
            on_error: None,
        };
        if let Err(error) = crate::database::drivers::postgresql::connection::connect_options(
            settings,
            &options,
            settings.database.as_deref().unwrap_or("postgres"),
        ) {
            let field = if settings.user.is_empty() {
                TestField::User
            } else {
                TestField::Tls
            };
            report(&reporter, &[field], TestStatus::Error, &error);
            return Err(error);
        }
    }
    let tunnel = if let Some(ssh) = &connection.ssh {
        check_endpoint(&reporter, deadline, &ssh.host, ssh.port, true).await?;
        let mut fields = vec![TestField::SshUser];
        match &ssh.auth {
            SshAuth::Password { .. } => fields.push(TestField::SshPassword),
            SshAuth::Key { .. } => fields.extend([TestField::SshKey, TestField::SshPassphrase]),
        }
        let ssh = ssh.clone();
        let remote_host = host.to_owned();
        let remaining = deadline.saturating_duration_since(Instant::now());
        Some(
            step(
                &reporter,
                deadline,
                &fields,
                "Authenticating SSH and opening tunnel…",
                "SSH tunnel ready",
                async move {
                    tokio::task::spawn_blocking(move || {
                        SshTunnel::new_with_timeout(&ssh, &remote_host, port, remaining)
                    })
                    .await
                    .map_err(|_| "SSH tunnel task failed".to_string())?
                },
            )
            .await?,
        )
    } else {
        check_endpoint(&reporter, deadline, host, port, false).await?;
        None
    };

    let mut fields = vec![TestField::User, TestField::Password];
    if tunnel.is_some() {
        // The remote database address is resolved by the SSH server. A listening
        // local forwarding port cannot prove the remote host/port are reachable.
        fields.extend([TestField::Host, TestField::Port]);
    }
    if connection.database.configured_database().is_some() {
        fields.push(TestField::Database);
    }
    if connection.database.engine() == DatabaseEngine::PostgreSql {
        fields.push(TestField::Tls);
    }
    report(
        &reporter,
        &fields,
        TestStatus::Checking,
        "Authenticating and opening database…",
    );
    let connected = Arc::new(AtomicBool::new(false));
    let failed_fields = Arc::new(Mutex::new(fields.clone()));
    let on_error = {
        let failed_fields = failed_fields.clone();
        let engine = connection.database.engine();
        Arc::new(move |error: &sqlx::Error| {
            // Use server error codes, not message matching, to distinguish a
            // rejected login from a missing/inaccessible database or TLS error.
            let code = error.as_database_error().and_then(|error| error.code());
            let mysql_number = error
                .as_database_error()
                .and_then(|error| error.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
                .map(|error| error.number());
            let fields = match (engine, mysql_number, code.as_deref()) {
                (DatabaseEngine::MySql, Some(1045 | 1698), _)
                | (DatabaseEngine::PostgreSql, _, Some("28P01" | "28000")) => {
                    Some(vec![TestField::User, TestField::Password])
                }
                (DatabaseEngine::MySql, Some(1044 | 1049), _)
                | (DatabaseEngine::PostgreSql, _, Some("3D000" | "42501")) => {
                    Some(vec![TestField::Database])
                }
                _ if matches!(error, sqlx::Error::Tls(_)) => Some(vec![TestField::Tls]),
                _ => None,
            };
            if let Some(fields) = fields {
                *failed_fields.lock().unwrap() = fields;
            }
        }) as Arc<dyn Fn(&sqlx::Error) + Send + Sync>
    };
    let on_connected = {
        let reporter = reporter.clone();
        let fields = fields.clone();
        let connected = connected.clone();
        Arc::new(move || {
            connected.store(true, Ordering::Relaxed);
            report(
                &reporter,
                &fields,
                TestStatus::Success,
                "Connection settings accepted",
            );
            report(
                &reporter,
                &[],
                TestStatus::Checking,
                "Reading server information…",
            );
        }) as Arc<dyn Fn() + Send + Sync>
    };
    let endpoint = tunnel
        .as_ref()
        .map(|t| ("127.0.0.1", t.local_port))
        .unwrap_or((host, port));
    let result = timeout_at(
        deadline,
        registry::open(
            &connection.database,
            ConnectOptions {
                read_only: !connection.allow_writes,
                endpoint: Some(endpoint),
                timeout_secs,
                tunneled: tunnel.is_some(),
                on_connected: Some(on_connected),
                on_error: Some(on_error),
            },
        ),
    )
    .await
    .unwrap_or_else(|_| Err("Connection test timed out waiting for the database server".into()));
    match result {
        Ok(opened) => {
            opened.driver.close().await;
            drop(tunnel);
            report(
                &reporter,
                &[],
                TestStatus::Success,
                "Connected successfully",
            );
            Ok("Connected successfully".into())
        }
        Err(error) => {
            let fields = if connected.load(Ordering::Relaxed) {
                vec![]
            } else {
                failed_fields.lock().unwrap().clone()
            };
            report(&reporter, &fields, TestStatus::Error, &error);
            Err(error)
        }
    }
}
