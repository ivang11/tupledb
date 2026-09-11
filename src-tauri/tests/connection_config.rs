use app_lib::connections::{Connection, DatabaseEngine, DatabaseSettings, SshAuth};
use app_lib::database::registry;
use serde_json::{json, Value};

fn legacy() -> Value {
    json!({
        "id": "c68c8fc5-984e-482c-83a1-7e3e366bbdf0", "name": "Local", "environment": "LOCAL",
        "mysql": {"host": "localhost", "port": 3306, "user": "root", "password": " test password ", "database": "app"},
        "ssh": {"host": "bastion", "port": 22, "user": "dev", "auth": {"type": "password", "password": "ssh-test"}}
    })
}

#[test]
fn migrates_legacy_connections_without_losing_settings_or_secrets() {
    let connection: Connection = serde_json::from_value(legacy()).unwrap();
    assert!(connection.allow_writes);
    assert_eq!(connection.database.engine(), DatabaseEngine::MySql);
    let serialized = serde_json::to_value(&connection).unwrap();
    assert!(serialized.get("mysql").is_none());
    assert_eq!(serialized["database"]["engine"], "mysql");
    assert_eq!(serialized["database"]["settings"], legacy()["mysql"]);
    assert_eq!(serialized["ssh"], legacy()["ssh"]);
    let round_trip: Connection = serde_json::from_value(serialized).unwrap();
    assert_eq!(round_trip.id, connection.id);
    assert_eq!(round_trip.database.configured_database(), Some("app"));
}

#[test]
fn retains_read_only_policy_during_migration() {
    let mut input = legacy();
    input["allow_writes"] = json!(false);
    assert!(
        !serde_json::from_value::<Connection>(input)
            .unwrap()
            .allow_writes
    );
}

#[test]
fn reads_engine_specific_settings_without_mysql_fields() {
    for settings in [
        json!({"engine": "sqlite", "settings": {"path": "/tmp/test.db", "read_only": true}}),
        json!({"engine": "postgresql", "settings": {"host": "localhost", "port": 5432, "user": "postgres", "database": "app", "ssl_mode": "verify_full"}}),
    ] {
        let mut input = legacy();
        input.as_object_mut().unwrap().remove("mysql");
        input["database"] = settings.clone();
        let connection: Connection = serde_json::from_value(input).unwrap();
        assert_eq!(
            serde_json::to_value(connection).unwrap()["database"]["engine"],
            settings["engine"]
        );
    }
}

#[test]
fn postgresql_database_can_be_omitted_null_or_empty_without_becoming_configured() {
    for database in [
        None,
        Some(Value::Null),
        Some(json!("")),
        Some(json!("selected database")),
    ] {
        let mut settings = json!({"engine":"postgresql", "settings": {
            "host":"localhost", "port":5432, "user":"postgres", "ssl_mode":"prefer"
        }});
        if let Some(database) = database {
            settings["settings"]["database"] = database;
        }
        let connection: DatabaseSettings = serde_json::from_value(settings.clone()).unwrap();
        let expected = settings["settings"]["database"]
            .as_str()
            .filter(|db| !db.is_empty());
        assert_eq!(connection.configured_database(), expected);
        let restored: DatabaseSettings =
            serde_json::from_value(serde_json::to_value(&connection).unwrap()).unwrap();
        assert_eq!(restored.configured_database(), expected);
    }
}

#[test]
fn rejects_ambiguous_missing_and_unknown_engines() {
    let mut input = legacy();
    input["database"] = json!({"engine": "sqlite", "settings": {"path": "/tmp/test.db"}});
    assert!(serde_json::from_value::<Connection>(input.clone()).is_err());
    input.as_object_mut().unwrap().remove("mysql");
    input["database"]["engine"] = json!("unknown");
    assert!(serde_json::from_value::<Connection>(input.clone()).is_err());
    input.as_object_mut().unwrap().remove("database");
    assert!(serde_json::from_value::<Connection>(input).is_err());
}

#[test]
fn restores_hidden_passwords_for_edit_and_test_without_trimming() {
    let stored: Connection = serde_json::from_value(legacy()).unwrap();
    let mut edited = stored.clone();
    edited.database.strip_password();
    if let SshAuth::Password { password } = &mut edited.ssh.as_mut().unwrap().auth {
        password.clear();
    }
    edited.preserve_secrets_from(&stored);
    assert_eq!(
        serde_json::to_value(edited).unwrap(),
        serde_json::to_value(stored).unwrap()
    );
}

#[test]
fn never_copies_database_passwords_between_engines() {
    let stored: Connection = serde_json::from_value(legacy()).unwrap();
    let mut postgres: DatabaseSettings = serde_json::from_value(json!({
        "engine": "postgresql", "settings": {"host": "localhost", "port": 5432, "user": "postgres", "database": "app", "ssl_mode": "prefer"}
    })).unwrap();
    postgres.preserve_password_from(&stored.database);
    assert!(serde_json::to_value(postgres).unwrap()["settings"]["password"].is_null());
}

#[tokio::test]
async fn unavailable_engines_are_rejected_before_opening_ssh_or_files() {
    assert_eq!(registry::AVAILABLE_DRIVERS.len(), 2);
    assert_eq!(registry::AVAILABLE_DRIVERS[0].engine, DatabaseEngine::MySql);
    assert!(registry::ensure_available(DatabaseEngine::PostgreSql).is_ok());
    assert!(registry::ensure_available(DatabaseEngine::Sqlite).is_err());
    let mut input = legacy();
    input.as_object_mut().unwrap().remove("mysql");
    input["database"] = json!({"engine": "sqlite", "settings": {"path": "/unavailable/path.db"}});
    let connection = serde_json::from_value(input).unwrap();
    let error = app_lib::services::connections::open(&connection)
        .await
        .err()
        .unwrap();
    assert!(error.contains("not implemented yet"));
}
