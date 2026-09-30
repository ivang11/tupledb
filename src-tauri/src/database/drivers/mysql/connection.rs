use super::MySqlDriver;
use crate::connections::MySqlSettings;
use crate::database::registry::{ConnectOptions, OpenedDatabase};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
use std::sync::Arc;
use std::time::Duration;

pub async fn open(
    settings: &MySqlSettings,
    options: ConnectOptions<'_>,
) -> Result<OpenedDatabase, String> {
    let (host, port) = options
        .endpoint
        .ok_or("MySQL requires a network endpoint")?;
    let mut opts = MySqlConnectOptions::new()
        .host(host)
        .port(port)
        .username(&settings.user)
        .ssl_mode(MySqlSslMode::Disabled);
    if let Some(password) = settings.password.as_deref().filter(|s| !s.is_empty()) {
        opts = opts.password(password);
    }
    if let Some(database) = settings.database.as_deref().filter(|s| !s.is_empty()) {
        opts = opts.database(database);
    }
    let mut pool_options = MySqlPoolOptions::new()
        .acquire_timeout(Duration::from_secs(options.timeout_secs))
        .test_before_acquire(true);
    if options.tunneled {
        // Preserve the existing SSH pool policy for servers that close parallel
        // forwarded channels early.
        pool_options = pool_options
            .max_connections(1)
            .min_connections(0)
            .idle_timeout(Duration::from_secs(60))
            .max_lifetime(Duration::from_secs(15 * 60));
    }
    let pool = pool_options.connect_with(opts).await.map_err(|e| {
        if let Some(on_error) = &options.on_error {
            on_error(&e);
        }
        format!("MySQL connection failed: {e}")
    })?;
    if let Some(on_connected) = &options.on_connected {
        on_connected();
    }
    let server_version = sqlx::query_scalar::<_, String>("SELECT VERSION()")
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|_| "Unknown".into());
    let no_group_by_check = sqlx::query_scalar::<_, String>("SELECT @@SESSION.sql_mode")
        .fetch_one(&pool)
        .await
        .map(|mode| mode.contains("ONLY_FULL_GROUP_BY"))
        .unwrap_or(false);
    Ok(OpenedDatabase {
        driver: Arc::new(MySqlDriver::new(pool, no_group_by_check)),
        server_version,
    })
}
