use crate::database::driver::*;
use crate::services::transfers::{
    export_database_file, export_table_file, import_sql_file, ExportOptions,
};
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ExportProgress {
    current: usize,
    total: usize,
    status: String,
}

#[tauri::command]
pub async fn export_table(
    window: tauri::Window,
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
    format: String,
    path: String,
) -> Result<usize, String> {
    let table = table.resolve(&database)?;
    use tauri::Emitter;

    let driver = state.get_driver(&connection_id)?;
    export_table_file(
        driver,
        database,
        table,
        format,
        path,
        &|current, total, status| {
            let _ = window.emit(
                "export-progress",
                ExportProgress {
                    current,
                    total,
                    status,
                },
            );
        },
    )
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn export_database(
    window: tauri::Window,
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    mode: String,
    path: String,
    tables: Option<Vec<TableTarget>>,
    export_id: Option<String>,
    format: Option<String>,
    drop_if_exists: Option<bool>,
    include_views: Option<bool>,
    use_transactions: Option<bool>,
    compress_gzip: Option<bool>,
) -> Result<usize, String> {
    use tauri::Emitter;

    let tables = tables
        .map(|items| {
            items
                .into_iter()
                .map(|t| t.resolve(&database))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let eid = export_id.unwrap_or_default();
    let fmt = format.as_deref().unwrap_or("sql");
    let options = ExportOptions {
        drop_if_exists: drop_if_exists.unwrap_or(true),
        include_views: include_views.unwrap_or(true),
        use_transactions: use_transactions.unwrap_or(true),
        compress_gzip: compress_gzip.unwrap_or(false),
    };
    state.clear_export_cancel(&eid);

    let driver = state.get_driver(&connection_id)?;
    let result = export_database_file(
        driver,
        database,
        mode,
        path,
        tables,
        fmt,
        options,
        &|progress| {
            let _ = window.emit("export-progress", progress);
        },
        &|| state.is_export_canceled(&eid),
    )
    .await;

    state.clear_export_cancel(&eid);
    result
}

#[tauri::command]
pub async fn cancel_export(state: State<'_, AppState>, export_id: String) -> Result<(), String> {
    state.request_export_cancel(&export_id);
    Ok(())
}

#[tauri::command]
pub async fn import_sql(
    window: tauri::Window,
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    path: String,
    import_id: String,
) -> Result<ImportResult, String> {
    let allow_writes = {
        let configs = state.connections_config.read();
        configs
            .get(&connection_id)
            .map(|c| c.allow_writes)
            .unwrap_or(true)
    };
    crate::security::ensure_writes_allowed(allow_writes)?;

    state.clear_import_cancel(&import_id);

    let driver = state.get_driver(&connection_id)?;
    let result = import_sql_file(
        driver,
        &database,
        &path,
        &import_id,
        &|| state.is_import_canceled(&import_id),
        &|progress| {
            use tauri::Emitter;
            let _ = window.emit("import-progress", progress);
        },
    )
    .await;
    state.clear_import_cancel(&import_id);
    result
}

#[tauri::command]
pub async fn cancel_import(
    state: State<'_, AppState>,
    connection_id: Uuid,
    import_id: String,
) -> Result<(), String> {
    state.request_import_cancel(&import_id);
    let driver = state.get_driver(&connection_id)?;
    driver.cancel_import(&import_id).await
}
