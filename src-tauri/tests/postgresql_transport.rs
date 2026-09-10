//! Run with `node scripts/test-postgresql-transport.mjs` (disposable Docker server).
use app_lib::connections::{
    Connection, DatabaseSettings, Environment, PostgreSqlSettings, PostgreSqlSslMode, SshAuth,
    SshSettings,
};
use app_lib::database::driver::{DatabaseDriver, SqlExportOptions};
use app_lib::services::connections;
use std::{io::Cursor, sync::Arc};
use uuid::Uuid;

fn fixture(name: &str) -> String {
    std::env::var(format!("TUPLEDB_TRANSPORT_{name}"))
        .expect("Run node scripts/test-postgresql-transport.mjs to provision the isolated fixture")
}

fn ca(name: &str) -> String {
    std::fs::read_to_string(fixture(name)).unwrap()
}

fn connection(host: &str, mode: PostgreSqlSslMode, cert: Option<String>, ssh: bool) -> Connection {
    Connection {
        id: Uuid::new_v4(),
        name: "Disposable transport regression".into(),
        environment: Environment::Local,
        database: DatabaseSettings::PostgreSql(PostgreSqlSettings {
            host: host.into(),
            port: if ssh {
                5432
            } else {
                fixture("PG_PORT").parse().unwrap()
            },
            user: "postgres".into(),
            password: Some("tupledb_transport_test".into()),
            database: None,
            ssl_mode: mode,
            ssl_root_cert: cert,
        }),
        ssh: ssh.then(|| SshSettings {
            host: "127.0.0.1".into(),
            port: fixture("SSH_PORT").parse().unwrap(),
            user: "root".into(),
            auth: SshAuth::Key {
                private_key_path: fixture("SSH_KEY"),
                passphrase: None,
            },
        }),
        timeout_secs: Some(5),
        allow_writes: true,
    }
}

async fn encrypted(driver: &Arc<dyn DatabaseDriver>, database: Option<&str>) -> bool {
    let result = driver
        .execute_query(
            database,
            "SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    result.rows[0]
        .get("ssl")
        .or_else(|| result.rows[0].get(0))
        .unwrap()
        .as_bool()
        .unwrap()
}

async fn roundtrip(driver: &Arc<dyn DatabaseDriver>) {
    let src = format!("transport_src_{}", Uuid::new_v4().simple());
    let dst = format!("transport_dst_{}", Uuid::new_v4().simple());
    driver.create_database(&src, None, None).await.unwrap();
    driver.create_database(&dst, None, None).await.unwrap();
    // Newly opened catalog pools must inherit the TLS policy and SSH endpoint.
    assert!(encrypted(driver, Some(&src)).await);
    assert!(encrypted(driver, Some(&dst)).await);
    let sql = b"CREATE TABLE public.items(id int primary key, label text, amount numeric(12,2)); INSERT INTO public.items VALUES(1,'TLS and SSH',123.45);";
    driver
        .import_stream(
            &src,
            &mut Cursor::new(sql),
            sql.len(),
            "transport-source",
            &|| false,
            &|_, _, _| {},
        )
        .await
        .unwrap();
    let tables = driver
        .get_tables(&src)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.reference)
        .collect::<Vec<_>>();
    let mut dump = Vec::new();
    let rows = driver
        .export_sql(
            &src,
            &tables,
            &SqlExportOptions {
                mode: "full".into(),
                drop_if_exists: true,
                use_transactions: true,
            },
            &mut dump,
            &|| false,
            &|_, _, _| {},
        )
        .await
        .unwrap();
    assert_eq!(rows, 1);
    let len = dump.len();
    driver
        .import_stream(
            &dst,
            &mut Cursor::new(dump),
            len,
            "transport-restore",
            &|| false,
            &|_, _, _| {},
        )
        .await
        .unwrap();
    let result = driver
        .execute_query(
            Some(&dst),
            "SELECT label || ':' || amount::text AS value FROM public.items",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        result.rows[0]
            .get("value")
            .or_else(|| result.rows[0].get(0))
            .unwrap()
            .as_str()
            .unwrap(),
        "TLS and SSH:123.45"
    );
    driver.drop_database(&src).await.unwrap();
    driver.drop_database(&dst).await.unwrap();
}

async fn accepts(connection: Connection, expect_encrypted: bool, transfer: bool) {
    let (session, _) = connections::open(&connection)
        .await
        .unwrap_or_else(|e| panic!("Unexpected connection failure: {e}"));
    assert_eq!(encrypted(&session.driver, None).await, expect_encrypted);
    if transfer {
        roundtrip(&session.driver).await;
    }
    connections::close(session).await;
}

async fn rejects(connection: Connection, reason: &str) {
    match connections::open(&connection).await {
        Ok((session, _)) => {
            connections::close(session).await;
            panic!("Connection unexpectedly accepted; expected {reason}");
        }
        Err(error) => assert!(error.contains(reason), "Expected {reason}, got {error}"),
    }
}

#[tokio::test]
#[ignore = "requires disposable TLS/SSH fixture; run scripts/test-postgresql-transport.mjs"]
async fn transport_encryption_modes() {
    use PostgreSqlSslMode::*;
    accepts(connection("127.0.0.1", Disable, None, false), false, false).await;
    accepts(connection("127.0.0.1", Prefer, None, false), true, false).await;
    accepts(connection("127.0.0.1", Require, None, false), true, false).await;
}

#[tokio::test]
#[ignore = "requires disposable TLS/SSH fixture; run scripts/test-postgresql-transport.mjs"]
async fn transport_verify_ca_checks_trust_without_checking_hostname() {
    use PostgreSqlSslMode::VerifyCa;
    // Certificate has ONLY DNS:localhost; the IP deliberately does not match.
    accepts(
        connection("127.0.0.1", VerifyCa, Some(ca("CA")), false),
        true,
        true,
    )
    .await;
    rejects(
        connection("127.0.0.1", VerifyCa, Some(ca("WRONG_CA")), false),
        "UnknownIssuer",
    )
    .await;
    rejects(
        connection("127.0.0.1", VerifyCa, None, false),
        "requires a CA",
    )
    .await;
    rejects(
        connection("127.0.0.1", VerifyCa, Some("invalid PEM".into()), false),
        "UnknownIssuer",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires disposable TLS/SSH fixture; run scripts/test-postgresql-transport.mjs"]
async fn transport_verify_full_checks_both_trust_and_hostname() {
    use PostgreSqlSslMode::VerifyFull;
    accepts(
        connection("localhost", VerifyFull, Some(ca("CA")), false),
        true,
        true,
    )
    .await;
    rejects(
        connection("127.0.0.1", VerifyFull, Some(ca("CA")), false),
        "certificate not valid for name",
    )
    .await;
    rejects(
        connection("localhost", VerifyFull, Some(ca("WRONG_CA")), false),
        "UnknownIssuer",
    )
    .await;
    rejects(
        connection("localhost", VerifyFull, None, false),
        "requires a CA",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires disposable TLS/SSH fixture; run scripts/test-postgresql-transport.mjs"]
async fn transport_ssh_retains_certificate_verification_across_catalogs() {
    use PostgreSqlSslMode::*;
    accepts(connection("127.0.0.1", Require, None, true), true, true).await;
    accepts(
        connection("127.0.0.1", VerifyCa, Some(ca("CA")), true),
        true,
        true,
    )
    .await;
    rejects(
        connection("127.0.0.1", VerifyCa, Some(ca("WRONG_CA")), true),
        "UnknownIssuer",
    )
    .await;
    rejects(
        connection("127.0.0.1", VerifyFull, Some(ca("CA")), true),
        "verify_full through SSH is not supported",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires expired certificate fixture; run scripts/test-postgresql-transport.mjs"]
async fn expired_certificate_is_rejected_even_with_a_trusted_ca() {
    use PostgreSqlSslMode::*;
    rejects(
        connection("127.0.0.1", VerifyCa, Some(ca("CA")), false),
        "certificate expired",
    )
    .await;
    rejects(
        connection("localhost", VerifyFull, Some(ca("CA")), false),
        "certificate expired",
    )
    .await;
    rejects(
        connection("127.0.0.1", VerifyCa, Some(ca("CA")), true),
        "certificate expired",
    )
    .await;
}
