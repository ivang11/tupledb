//! Cancellation shared by editor queries, SQL import and SQL export.
use sqlx::{Connection, PgConnection, PgPool};
use std::time::Duration;

// The caller must own the original session until this finishes. Matching both
// PID and backend start protects against PID reuse. The control session is
// independent of the pool, including for one-slot pools and forwarded SSH ports.
pub(super) async fn cancel_backend(pool: &PgPool, database: &str, pid: i32, started: &str) {
    let _ = tokio::time::timeout(Duration::from_secs(5), async {
        let mut control = PgConnection::connect_with(pool.connect_options().as_ref()).await?;
        let result = sqlx::query("SELECT pg_cancel_backend(pid) FROM pg_stat_activity WHERE pid=$1 AND backend_start=$2::timestamptz AND datname=$3")
            .bind(pid).bind(started).bind(database).execute(&mut control).await;
        let _ = control.close().await;
        result
    }).await;
    // Callers close their disposable session even if control connection fails.
}
