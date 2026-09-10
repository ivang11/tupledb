use super::PostgreSqlDriver;
use crate::connections::{PostgreSqlSettings, PostgreSqlSslMode};
use crate::database::registry::{ConnectOptions, OpenedDatabase};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use std::{sync::Arc, time::Duration};

pub fn connect_options(
    settings: &PostgreSqlSettings,
    options: &ConnectOptions<'_>,
    database: &str,
) -> Result<PgConnectOptions, String> {
    if settings.user.is_empty() {
        return Err("Enter a PostgreSQL user name".into());
    }
    // A forwarded localhost endpoint does not match the remote certificate name.
    // Fail closed until independent TLS server-name configuration is available.
    if options.tunneled && matches!(settings.ssl_mode, PostgreSqlSslMode::VerifyFull) {
        return Err(
            "PostgreSQL verify_full through SSH is not supported yet; use a direct TLS connection"
                .into(),
        );
    }
    let (host, port) = options
        .endpoint
        .ok_or("PostgreSQL requires a network endpoint")?;
    let ssl = match settings.ssl_mode {
        PostgreSqlSslMode::Disable => PgSslMode::Disable,
        PostgreSqlSslMode::Prefer => PgSslMode::Prefer,
        PostgreSqlSslMode::Require => PgSslMode::Require,
        PostgreSqlSslMode::VerifyCa => PgSslMode::VerifyCa,
        PostgreSqlSslMode::VerifyFull => PgSslMode::VerifyFull,
    };
    let mut opts = PgConnectOptions::new()
        .host(host)
        .port(port)
        .username(&settings.user)
        .password(settings.password.as_deref().unwrap_or(""))
        .database(database)
        .ssl_mode(ssl)
        .application_name("TupleDB")
        .options([
            // No server-side statement_timeout: long exports/imports and analytical
            // queries run through this driver's own pg_cancel_backend-based
            // cancellation (session.rs) instead of a blind wall-clock cutoff.
            ("statement_timeout", "0"),
            ("idle_in_transaction_session_timeout", "120000"),
            (
                "default_transaction_read_only",
                if options.read_only { "on" } else { "off" },
            ),
        ]);
    // verify-ca/verify-full only pass if the server's certificate chains to a CA
    // the OS already trusts, which a self-signed/private-CA PostgreSQL server
    // (the common case outside managed cloud providers) never does. Fail with a
    // clear message instead of a confusing TLS handshake error.
    if matches!(
        settings.ssl_mode,
        PostgreSqlSslMode::VerifyCa | PostgreSqlSslMode::VerifyFull
    ) {
        let cert = settings
            .ssl_root_cert
            .as_deref()
            .map(str::trim)
            .filter(|cert| !cert.is_empty())
            .ok_or(
                "This TLS mode requires a CA certificate. Paste the PEM certificate that \
                 signed the server's certificate, or switch to \"Require encryption\" if \
                 you don't have one.",
            )?;
        opts = opts.ssl_root_cert_from_pem(cert.as_bytes().to_vec());
    }
    Ok(opts)
}

fn initial_catalogs(settings: &PostgreSqlSettings) -> Vec<String> {
    if let Some(database) = settings.database.as_deref().filter(|db| !db.is_empty()) {
        return vec![database.to_owned()];
    }
    let mut catalogs = vec!["postgres".to_owned()];
    for candidate in [&settings.user, "template1"] {
        if !catalogs.iter().any(|db| db == candidate) {
            catalogs.push(candidate.to_owned());
        }
    }
    catalogs
}

fn may_try_another_catalog(error: &sqlx::Error) -> bool {
    // Do not retry authentication, TLS or network failures against other databases.
    matches!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("3D000" | "42501")
    )
}

pub async fn open(
    settings: &PostgreSqlSettings,
    options: ConnectOptions<'_>,
) -> Result<OpenedDatabase, String> {
    let pool_options = PgPoolOptions::new()
        .max_connections(if options.tunneled { 1 } else { 5 })
        .acquire_timeout(Duration::from_secs(options.timeout_secs))
        .idle_timeout(Duration::from_secs(60));
    let mut connected = None;
    let mut last_error = String::new();
    for catalog in initial_catalogs(settings) {
        let opts = connect_options(settings, &options, &catalog)?;
        match pool_options.clone().connect_with(opts).await {
            Ok(pool) => {
                connected = Some((pool, catalog));
                break;
            }
            Err(error) => {
                last_error = format!("PostgreSQL database {catalog:?}: {error}");
                if !may_try_another_catalog(&error) {
                    return Err(last_error);
                }
            }
        }
    }
    let (pool, catalog) = connected.ok_or_else(|| format!(
        "Could not open an initial PostgreSQL database. Specify an existing database you can access. {last_error}"
    ))?;
    let version: String = sqlx::query_scalar("SHOW server_version")
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(OpenedDatabase {
        driver: Arc::new(PostgreSqlDriver::new(pool, catalog, options.read_only)),
        server_version: version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honors_endpoint_database_and_each_tls_mode() {
        for (mode, expected) in [
            (PostgreSqlSslMode::Disable, PgSslMode::Disable),
            (PostgreSqlSslMode::Prefer, PgSslMode::Prefer),
            (PostgreSqlSslMode::Require, PgSslMode::Require),
            (PostgreSqlSslMode::VerifyCa, PgSslMode::VerifyCa),
            (PostgreSqlSslMode::VerifyFull, PgSslMode::VerifyFull),
        ] {
            let needs_cert = matches!(
                mode,
                PostgreSqlSslMode::VerifyCa | PostgreSqlSslMode::VerifyFull
            );
            let settings = PostgreSqlSettings {
                host: "remote.example".into(),
                port: 5432,
                user: "tester".into(),
                password: Some(" password ".into()),
                database: Some("app".into()),
                ssl_mode: mode,
                ssl_root_cert: needs_cert.then(|| "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----".into()),
            };
            let options = ConnectOptions {
                endpoint: Some(("127.0.0.1", 6543)),
                timeout_secs: 5,
                tunneled: false,
                read_only: true,
            };
            let actual = connect_options(&settings, &options, "app").unwrap();
            assert_eq!(actual.get_host(), "127.0.0.1");
            assert_eq!(actual.get_port(), 6543);
            assert_eq!(actual.get_database(), Some("app"));
            assert_eq!(
                std::mem::discriminant(&actual.get_ssl_mode()),
                std::mem::discriminant(&expected)
            );
            assert!(actual
                .get_options()
                .unwrap()
                .contains("statement_timeout=0"));
            assert!(actual
                .get_options()
                .unwrap()
                .contains("default_transaction_read_only=on"));
        }
    }

    #[test]
    fn accepts_optional_database_but_does_not_downgrade_verify_full_over_ssh() {
        let mut settings = PostgreSqlSettings {
            host: "remote.example".into(),
            port: 5432,
            user: "tester".into(),
            password: None,
            database: Some("app".into()),
            ssl_mode: PostgreSqlSslMode::VerifyFull,
            ssl_root_cert: None,
        };
        let mut options = ConnectOptions {
            endpoint: Some(("127.0.0.1", 6543)),
            timeout_secs: 5,
            tunneled: true,
            read_only: false,
        };
        assert!(connect_options(&settings, &options, "app")
            .unwrap_err()
            .contains("verify_full"));
        settings.database = None;
        options.tunneled = false;
        settings.ssl_root_cert = Some("-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----".into());
        assert_eq!(
            connect_options(&settings, &options, "postgres")
                .unwrap()
                .get_database(),
            Some("postgres")
        );
        assert_eq!(
            initial_catalogs(&settings),
            vec!["postgres", "tester", "template1"]
        );
        settings.database = Some(String::new());
        assert_eq!(
            initial_catalogs(&settings),
            vec!["postgres", "tester", "template1"]
        );
        settings.database = Some("explicit database".into());
        assert_eq!(initial_catalogs(&settings), vec!["explicit database"]);
        settings.user.clear();
        assert!(connect_options(&settings, &options, "postgres").is_err());
    }

    #[test]
    fn verify_ca_and_verify_full_require_a_ca_certificate() {
        let settings = PostgreSqlSettings {
            host: "remote.example".into(),
            port: 5432,
            user: "tester".into(),
            password: None,
            database: Some("app".into()),
            ssl_mode: PostgreSqlSslMode::VerifyCa,
            ssl_root_cert: None,
        };
        let options = ConnectOptions {
            endpoint: Some(("remote.example", 5432)),
            timeout_secs: 5,
            tunneled: false,
            read_only: false,
        };
        let error = connect_options(&settings, &options, "app").unwrap_err();
        assert!(error.contains("CA certificate"));

        let mut settings = settings;
        settings.ssl_root_cert = Some("   ".into());
        assert!(connect_options(&settings, &options, "app")
            .unwrap_err()
            .contains("CA certificate"));

        settings.ssl_root_cert = Some("-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----".into());
        assert!(connect_options(&settings, &options, "app").is_ok());

        // Modes below verify-ca never need a certificate.
        settings.ssl_mode = PostgreSqlSslMode::Require;
        settings.ssl_root_cert = None;
        assert!(connect_options(&settings, &options, "app").is_ok());
    }
}
