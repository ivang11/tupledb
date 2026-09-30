use app_lib::connections::{Connection, DatabaseSettings};
use app_lib::services::connection_test::{
    self, ConnectionTestProgress, ProgressReporter, TestField, TestStatus,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use uuid::Uuid;

fn connection(host: &str, port: u16) -> Connection {
    serde_json::from_value(json!({
        "id": Uuid::new_v4(), "name": "Progress fixture", "environment": "LOCAL", "timeout_secs": 1,
        "database": {"engine":"mysql", "settings": {"host":host,"port":port,"user":"fixture","password":"fixture-secret"}}
    })).unwrap()
}

fn recorder() -> (Arc<Mutex<Vec<ConnectionTestProgress>>>, ProgressReporter) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let capture = events.clone();
    (
        events,
        Arc::new(move |event| capture.lock().unwrap().push(event)),
    )
}

fn has(events: &[ConnectionTestProgress], field: TestField, status: TestStatus) -> bool {
    events
        .iter()
        .any(|event| event.fields.contains(&field) && event.status == status)
}

#[tokio::test]
async fn a_refused_port_preserves_host_success_and_never_validates_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let (events, reporter) = recorder();
    assert!(
        connection_test::test(&connection("127.0.0.1", port), reporter)
            .await
            .is_err()
    );
    let events = events.lock().unwrap();
    assert!(has(&events, TestField::Host, TestStatus::Success));
    assert!(has(&events, TestField::Port, TestStatus::Error));
    assert!(!has(&events, TestField::User, TestStatus::Success));
    assert!(!serde_json::to_string(&*events)
        .unwrap()
        .contains("fixture-secret"));
}

#[tokio::test]
async fn a_nonresponsive_server_reports_reachability_before_the_shared_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let mut sockets = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            sockets.push(socket);
        }
    });
    let (events, reporter) = recorder();
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        connection_test::test(&connection("127.0.0.1", port), reporter),
    )
    .await;
    server.abort();
    assert!(result.unwrap().unwrap_err().contains("timed out"));
    let events = events.lock().unwrap();
    assert!(has(&events, TestField::Host, TestStatus::Success));
    assert!(has(&events, TestField::Port, TestStatus::Success));
    assert!(has(&events, TestField::User, TestStatus::Checking));
    assert!(has(&events, TestField::User, TestStatus::Error));
    assert!(!has(&events, TestField::Password, TestStatus::Success));
}

#[tokio::test]
async fn invalid_host_stops_before_port_or_authentication_checks() {
    let (events, reporter) = recorder();
    assert!(
        connection_test::test(&connection("invalid\0host", 3306), reporter)
            .await
            .is_err()
    );
    let events = events.lock().unwrap();
    assert!(has(&events, TestField::Host, TestStatus::Error));
    assert!(!events
        .iter()
        .any(|event| event.fields.contains(&TestField::Port)));
}

#[tokio::test]
async fn missing_tls_certificate_fails_before_network_checks() {
    let mut connection = connection("invalid\0host", 5432);
    connection.database = serde_json::from_value(json!({"engine":"postgresql", "settings": {
        "host":"invalid\0host", "port":5432, "user":"fixture", "ssl_mode":"verify_ca"
    }}))
    .unwrap();
    let (events, reporter) = recorder();
    assert!(connection_test::test(&connection, reporter)
        .await
        .unwrap_err()
        .contains("CA certificate"));
    let events = events.lock().unwrap();
    assert!(has(&events, TestField::Tls, TestStatus::Error));
    assert!(!events
        .iter()
        .any(|event| event.fields.contains(&TestField::Host)));
}

#[cfg(unix)]
#[tokio::test]
async fn nonresponsive_ssh_reports_ssh_fields_without_validating_the_remote_database() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let mut sockets = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            sockets.push(socket);
        }
    });
    let mut connection = connection("database.internal", 3306);
    connection.ssh = serde_json::from_value(json!({
        "host":"127.0.0.1", "port":port, "user":"fixture",
        "auth":{"type":"password", "password":"fixture-ssh-secret"}
    }))
    .unwrap();
    let (events, reporter) = recorder();
    let result = connection_test::test(&connection, reporter).await;
    server.abort();
    assert!(result.is_err());
    let events = events.lock().unwrap();
    assert!(has(&events, TestField::SshHost, TestStatus::Success));
    assert!(has(&events, TestField::SshPort, TestStatus::Success));
    assert!(has(&events, TestField::SshPassword, TestStatus::Error));
    assert!(!has(&events, TestField::Host, TestStatus::Success));
    assert!(!has(&events, TestField::Password, TestStatus::Success));
    assert!(!serde_json::to_string(&*events)
        .unwrap()
        .contains("fixture-ssh-secret"));
}

async fn database_cases(mut connection: Connection, database: &str) {
    connection.timeout_secs = Some(5);
    for configured_database in [None, Some(database)] {
        match &mut connection.database {
            DatabaseSettings::MySql(settings) => {
                settings.database = configured_database.map(str::to_owned)
            }
            DatabaseSettings::PostgreSql(settings) => {
                settings.database = configured_database.map(str::to_owned)
            }
            _ => unreachable!(),
        }
        let (events, reporter) = recorder();
        assert!(connection_test::test(&connection, reporter).await.is_ok());
        let events = events.lock().unwrap();
        assert!(has(&events, TestField::User, TestStatus::Success));
        assert!(has(&events, TestField::Password, TestStatus::Success));
        assert_eq!(
            has(&events, TestField::Database, TestStatus::Success),
            configured_database.is_some()
        );
        let accepted = events
            .iter()
            .position(|event| {
                event.fields.contains(&TestField::User) && event.status == TestStatus::Success
            })
            .unwrap();
        let metadata = events
            .iter()
            .position(|event| event.message == "Reading server information…")
            .unwrap();
        assert!(accepted < metadata);
        assert_eq!(events.last().unwrap().status, TestStatus::Success);
    }
    let valid = connection.clone();
    for wrong_password in [true, false] {
        connection = valid.clone();
        match &mut connection.database {
            DatabaseSettings::MySql(settings) => {
                if wrong_password {
                    settings.password = Some("wrong-fixture-password".into());
                } else {
                    settings.database = Some(format!("missing_{}", Uuid::new_v4().simple()));
                }
            }
            DatabaseSettings::PostgreSql(settings) => {
                if wrong_password {
                    settings.password = Some("wrong-fixture-password".into());
                } else {
                    settings.database = Some(format!("missing_{}", Uuid::new_v4().simple()));
                }
            }
            _ => unreachable!(),
        }
        let (events, reporter) = recorder();
        assert!(connection_test::test(&connection, reporter).await.is_err());
        let events = events.lock().unwrap();
        assert!(has(&events, TestField::Host, TestStatus::Success));
        assert!(has(&events, TestField::Port, TestStatus::Success));
        assert!(!has(&events, TestField::User, TestStatus::Success));
        assert!(!has(&events, TestField::Database, TestStatus::Success));
        if wrong_password {
            assert!(has(&events, TestField::Password, TestStatus::Error));
            assert!(!has(&events, TestField::Database, TestStatus::Error));
        } else {
            assert!(has(&events, TestField::Database, TestStatus::Error));
            assert!(!has(&events, TestField::Password, TestStatus::Error));
        }
    }
}

#[tokio::test]
#[ignore = "requires isolated TUPLEDB_TEST_POSTGRESQL_URL"]
async fn postgresql_progress_tracks_real_authentication_and_database_access() {
    use sqlx::postgres::PgConnectOptions;
    use std::str::FromStr;
    let url = std::env::var("TUPLEDB_TEST_POSTGRESQL_URL").unwrap();
    let opts = PgConnectOptions::from_str(&url).unwrap();
    let mut connection = connection(opts.get_host(), opts.get_port());
    connection.database = serde_json::from_value(json!({"engine":"postgresql", "settings": {
        "host":opts.get_host(), "port":opts.get_port(), "user":opts.get_username(),
        "password":"progress-fixture-password", "ssl_mode":"disable"
    }}))
    .unwrap();
    database_cases(connection, opts.get_database().unwrap_or("postgres")).await;
}

#[tokio::test]
#[ignore = "requires isolated TUPLEDB_TEST_MYSQL_URL"]
async fn mysql_progress_tracks_real_authentication_and_database_access() {
    use sqlx::mysql::MySqlConnectOptions;
    use std::str::FromStr;
    let url = std::env::var("TUPLEDB_TEST_MYSQL_URL").unwrap();
    let opts = MySqlConnectOptions::from_str(&url).unwrap();
    let mut connection = connection(opts.get_host(), opts.get_port());
    if let DatabaseSettings::MySql(settings) = &mut connection.database {
        settings.user = opts.get_username().into();
        settings.password = Some("progress-fixture-password".into());
    }
    database_cases(connection, opts.get_database().unwrap_or("mysql")).await;
}
