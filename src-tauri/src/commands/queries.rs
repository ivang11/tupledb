use super::results::compact_rows_for_ipc;
use crate::database::driver::*;
use crate::state::AppState;
use parking_lot::RwLock;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

/// Fire-and-forget: returns immediately and emits `query-result:{query_id}` when done.
/// This keeps the frontend UI responsive during long-running queries.
#[tauri::command]
pub fn execute_query(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: Option<String>,
    sql: String,
    query_id: String,
    max_retained_cells: Option<usize>,
    timeout_secs: Option<u64>,
) -> Result<(), String> {
    let (env, allow_writes) = {
        let configs = state.connections_config.read();
        configs
            .data()?
            .get(&connection_id)
            .map(|c| (c.environment, c.allow_writes))
            .unwrap_or((crate::connections::Environment::Local, true))
    };
    crate::security::is_query_safe(&sql, env, allow_writes)?;
    let driver = state.get_driver(&connection_id)?;
    let app_handle = state.app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let t0 = std::time::Instant::now();

        let progress_handle = app_handle.clone();
        let progress_qid = query_id.clone();
        let on_progress: Option<Arc<dyn Fn(u64) + Send + Sync>> =
            Some(Arc::new(move |rows: u64| {
                use tauri::Emitter;
                let _ = progress_handle.emit(
                    &format!("query-progress:{}", progress_qid),
                    serde_json::json!({ "rows_fetched": rows }),
                );
            }));

        let chunk_handle = app_handle.clone();
        let chunk_qid = query_id.clone();
        let chunk_columns = Arc::new(RwLock::new(Vec::<ColumnInfo>::new()));
        let chunk_columns_for_cb = Arc::clone(&chunk_columns);
        let on_chunk: Option<QueryChunkCallback> = Some(Arc::new(
            move |columns: Option<Vec<crate::database::driver::ColumnInfo>>,
                  rows: Vec<serde_json::Value>| {
                use tauri::Emitter;
                if let Some(ref incoming_columns) = columns {
                    *chunk_columns_for_cb.write() = incoming_columns.clone();
                }
                let compact_rows = compact_rows_for_ipc(&chunk_columns_for_cb.read(), rows);
                let _ = chunk_handle.emit(
                    &format!("query-chunk:{}", chunk_qid),
                    serde_json::json!({ "columns": columns, "rows": compact_rows }),
                );
            },
        ));

        let execution = driver
            .execute_query(
                database.as_deref(),
                &sql,
                Some(&query_id),
                on_progress,
                on_chunk,
                max_retained_cells,
            );
        tokio::pin!(execution);
        let result = tokio::select! {
            result = &mut execution => result,
            _ = tokio::time::sleep(std::time::Duration::from_secs(timeout_secs.unwrap_or(300).clamp(1, 3600))) => {
                let _ = driver.cancel_query(&query_id).await;
                Err("Query timed out".into())
            }
        };
        let ms = t0.elapsed().as_millis() as u64;

        use tauri::Emitter;

        // Send result to the waiting frontend listener.
        // For SELECT queries rows already arrived via query-chunk events, so we omit them.
        let payload = match &result {
            Ok(r) => serde_json::json!({
                "ok": {
                    "columns": r.columns,
                    "rows": serde_json::Value::Array(vec![]),
                    "rows_affected": r.rows_affected,
                    "is_select": r.is_select,
                },
                "duration_ms": ms,
                "streamed": r.is_select,
            }),
            Err(e) => serde_json::json!({ "error": e, "duration_ms": ms }),
        };
        let _ = app_handle.emit(&format!("query-result:{}", query_id), payload);

        // Query log
        let now = chrono::Local::now();
        let err_msg = result.err();
        let _ = app_handle.emit(
            "query-log",
            serde_json::json!({
                "connection_id": connection_id.to_string(),
                "database": database,
                "sql": sql,
                "timestamp": now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                "duration_ms": ms,
                "error": err_msg,
            }),
        );
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_query(
    state: State<'_, AppState>,
    connection_id: Uuid,
    query_id: String,
) -> Result<(), String> {
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(driver.capabilities().cancel_query, "cancel_query")?;
    driver.cancel_query(&query_id).await
}

#[tauri::command]
pub async fn execute_query_batch(
    state: State<'_, AppState>, connection_id: Uuid, database: Option<String>, statements: Vec<String>,
    query_id: String, row_limit: Option<usize>, timeout_secs: Option<u64>,
) -> Result<Vec<BatchQueryResult>, String> {
    if statements.is_empty() || statements.len() > 200 { return Err("Execute between 1 and 200 statements per batch".into()); }
    let (env, allow_writes) = { let configs = state.connections_config.read(); configs.data()?.get(&connection_id)
        .map(|c| (c.environment, c.allow_writes)).unwrap_or((crate::connections::Environment::Local, true)) };
    for statement in &statements { crate::security::is_query_safe(statement, env, allow_writes)?; }
    let driver = state.get_driver(&connection_id)?;
    let execution = driver.execute_query_batch(database.as_deref(), &statements, &query_id, row_limit.unwrap_or(5000).clamp(100, 50000), 300_000);
    tokio::pin!(execution);
    let results = tokio::select! {
        result = &mut execution => result,
        _ = tokio::time::sleep(std::time::Duration::from_secs(timeout_secs.unwrap_or(300).clamp(1, 3600))) => {
            let _ = driver.cancel_query(&query_id).await;
            Err("Query timed out".into())
        }
    }?;
    for result in &results { state.emit_query_log_context(Some(connection_id), database.as_deref(), &result.sql, result.duration_ms, result.error.as_deref()); }
    Ok(results.into_iter().map(|mut item| { if let Some(result) = &mut item.result { result.rows = compact_rows_for_ipc(&result.columns, std::mem::take(&mut result.rows)); } item }).collect())
}

#[tauri::command]
pub async fn keep_connection_alive(state: State<'_, AppState>, connection_id: Uuid) -> Result<(), String> {
    let driver = state.get_driver(&connection_id)?;
    driver.get_databases().await.map(|_| ())
}
