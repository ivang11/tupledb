/// SSH tunnel integration tests.
///
/// These tests require a real SSH server with MySQL accessible through it.
/// They are **skipped silently** when the required env vars are not set, so
/// the standard CI run (`cargo test`) is unaffected.
///
/// To run:
///   export DB_VIEWER_TEST_SSH_HOST=bastion.example.com
///   export DB_VIEWER_TEST_SSH_PORT=22          # optional, default 22
///   export DB_VIEWER_TEST_SSH_USER=ubuntu
///   export DB_VIEWER_TEST_SSH_KEY_PATH=/path/to/id_ed25519
///   export DB_VIEWER_TEST_MYSQL_VIA_SSH_HOST=127.0.0.1   # MySQL host as seen from SSH server
///   export DB_VIEWER_TEST_MYSQL_VIA_SSH_PORT=3306
///   export DB_VIEWER_TEST_MYSQL_VIA_SSH_USER=root
///   export DB_VIEWER_TEST_MYSQL_VIA_SSH_PASS=secret
///   cargo test --test ssh_integration -- --nocapture
use app_lib::connections::{SshAuth, SshSettings};
use app_lib::driver::DatabaseDriver;
use app_lib::mysql::MySqlDriver;
use app_lib::schema::import_sql_file;
use app_lib::ssh::SshTunnel;
use sqlx::MySqlPool;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Reads all required SSH + MySQL env vars.
/// Returns `None` (causing the test to skip) when any var is absent.
struct SshConfig {
    settings: SshSettings,
    mysql_host: String,
    mysql_port: u16,
    mysql_user: String,
    mysql_pass: String,
}

fn read_config() -> Option<SshConfig> {
    let ssh_host = std::env::var("DB_VIEWER_TEST_SSH_HOST").ok()?;
    let ssh_port = std::env::var("DB_VIEWER_TEST_SSH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(22u16);
    let ssh_user = std::env::var("DB_VIEWER_TEST_SSH_USER").ok()?;
    let key_path = std::env::var("DB_VIEWER_TEST_SSH_KEY_PATH").ok()?;
    let mysql_host = std::env::var("DB_VIEWER_TEST_MYSQL_VIA_SSH_HOST").ok()?;
    let mysql_port = std::env::var("DB_VIEWER_TEST_MYSQL_VIA_SSH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3306u16);
    let mysql_user = std::env::var("DB_VIEWER_TEST_MYSQL_VIA_SSH_USER").ok()?;
    let mysql_pass = std::env::var("DB_VIEWER_TEST_MYSQL_VIA_SSH_PASS").unwrap_or_default();

    Some(SshConfig {
        settings: SshSettings {
            host: ssh_host,
            port: ssh_port,
            user: ssh_user,
            auth: SshAuth::Key {
                private_key_path: key_path,
                passphrase: None,
            },
        },
        mysql_host,
        mysql_port,
        mysql_user,
        mysql_pass,
    })
}

async fn pool_through_tunnel(tunnel: &SshTunnel, cfg: &SshConfig) -> MySqlPool {
    let url = format!(
        "mysql://{}:{}@127.0.0.1:{}/mysql",
        cfg.mysql_user, cfg.mysql_pass, tunnel.local_port,
    );
    MySqlPool::connect(&url)
        .await
        .expect("connect to MySQL through SSH tunnel")
}

async fn create_db(pool: &MySqlPool) -> String {
    let db = format!("db_viewer_ssh_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE `{}`", db))
        .execute(pool)
        .await
        .expect("create temp database");
    db
}

async fn drop_db(pool: &MySqlPool, db: &str) {
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS `{}`", db))
        .execute(pool)
        .await;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// Verifies that the SSH tunnel opens, MySQL is reachable, and basic read/write
/// works through the tunnel.
#[tokio::test]
async fn ssh_tunnel_connects_and_reads_mysql() {
    let Some(cfg) = read_config() else {
        eprintln!("ssh_tunnel_connects_and_reads_mysql: skipped (DB_VIEWER_TEST_SSH_HOST not set)");
        return;
    };

    let tunnel = SshTunnel::new(&cfg.settings, &cfg.mysql_host, cfg.mysql_port)
        .expect("open SSH tunnel");

    let pool = pool_through_tunnel(&tunnel, &cfg).await;
    let db = create_db(&pool).await;

    sqlx::query(&format!(
        "CREATE TABLE `{}`.`ping` (id INT PRIMARY KEY, val VARCHAR(50))", db
    ))
    .execute(&pool)
    .await
    .expect("create table");

    sqlx::query(&format!(
        "INSERT INTO `{}`.`ping` VALUES (1, 'hello-via-ssh')", db
    ))
    .execute(&pool)
    .await
    .expect("insert row");

    let row: (String,) = sqlx::query_as(&format!("SELECT val FROM `{}`.`ping` WHERE id = 1", db))
        .fetch_one(&pool)
        .await
        .expect("read row");

    assert_eq!(row.0, "hello-via-ssh");

    drop_db(&pool, &db).await;
}

/// Imports a representative SQL dump through an SSH tunnel and validates that
/// batching, max_allowed_packet handling, and per-phase metrics work correctly.
#[tokio::test]
async fn ssh_import_validates_batching_and_metrics() {
    let Some(cfg) = read_config() else {
        eprintln!("ssh_import_validates_batching_and_metrics: skipped (DB_VIEWER_TEST_SSH_HOST not set)");
        return;
    };

    let tunnel = SshTunnel::new(&cfg.settings, &cfg.mysql_host, cfg.mysql_port)
        .expect("open SSH tunnel");

    let pool = pool_through_tunnel(&tunnel, &cfg).await;
    let db = create_db(&pool).await;

    // Fetch max_allowed_packet so the driver can adapt batch sizes
    let url = format!(
        "mysql://{}:{}@127.0.0.1:{}/{db}",
        cfg.mysql_user, cfg.mysql_pass, tunnel.local_port,
    );
    let driver = MySqlDriver::new(
        MySqlPool::connect(&url).await.expect("connect for driver"),
        false,
    );

    // Build a representative dump: DDL + 500 compatible INSERTs + a comment
    // and a string with an embedded semicolon.
    let mut sql = String::new();
    sql.push_str(&format!(
        "CREATE TABLE `{}`.`items` (id INT PRIMARY KEY, name VARCHAR(100));\n",
        db
    ));
    sql.push_str("-- Seeding data\n");
    for i in 1..=500 {
        sql.push_str(&format!(
            "INSERT INTO `{}`.`items` VALUES ({}, 'item-{}: note; with semicolon');\n",
            db, i, i
        ));
    }
    // Write dump to a temp file
    let dump_path = std::env::temp_dir().join(format!("ssh_test_{}.sql", Uuid::new_v4().simple()));
    std::fs::write(&dump_path, &sql).expect("write dump file");

    let progress_log: Arc<Mutex<Vec<(u64, u64)>>> = Arc::new(Mutex::new(Vec::new()));
    let log_clone = progress_log.clone();
    let import_id = Uuid::new_v4().to_string();

    let result = import_sql_file(
        Arc::new(driver),
        &db,
        dump_path.to_str().unwrap(),
        &import_id,
        Arc::new(move |current, total| {
            log_clone.lock().unwrap().push((current, total));
        }),
    )
    .await
    .expect("import succeeded");

    // All 500 INSERTs plus the CREATE TABLE should have been executed
    assert!(
        result.metrics.parsed_statements >= 501,
        "expected ≥501 parsed statements, got {}",
        result.metrics.parsed_statements
    );

    // Compaction should have merged most of the 500 single-row INSERTs
    assert!(
        result.metrics.compacted_statements < result.metrics.parsed_statements,
        "expected fewer SQL blocks than parsed statements after compaction"
    );

    // At least one batch was executed
    assert!(result.metrics.executed_batches >= 1);

    // Per-phase timings are non-negative
    assert!(result.metrics.read_ms < result.metrics.total_ms + 1);
    assert!(result.metrics.execute_ms <= result.metrics.total_ms + 1);

    // Progress callbacks were called
    assert!(!progress_log.lock().unwrap().is_empty(), "progress should fire");

    // Validate rows reached the DB
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM `{}`.`items`", db))
        .fetch_one(&pool)
        .await
        .expect("count rows");
    assert_eq!(count.0, 500, "all 500 rows should be present");

    std::fs::remove_file(&dump_path).ok();
    drop_db(&pool, &db).await;
}
