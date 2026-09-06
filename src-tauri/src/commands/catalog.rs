use crate::database::driver::*;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;
#[tauri::command]
pub async fn get_databases(
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<Vec<String>, String> {
    // If the connection is configured with a specific database, return only that one
    let configured_db = {
        let configs = state.connections_config.read();
        configs
            .get(&connection_id)
            .and_then(|c| c.database.configured_database().map(str::to_owned))
    };

    if let Some(db) = configured_db {
        return Ok(vec![db]);
    }

    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver.get_databases().await;
    let ms = t0.elapsed().as_millis() as u64;
    state.emit_query_log_context(
        Some(connection_id),
        None,
        "-- List databases",
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    result
}

#[tauri::command]
pub async fn get_database_creation_options(
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<DatabaseCreationOptions, String> {
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(
        driver.capabilities().database_collations,
        "get_database_creation_options",
    )?;
    driver.get_database_creation_options().await
}

#[tauri::command]
pub async fn create_database(
    state: State<'_, AppState>,
    connection_id: Uuid,
    name: String,
    character_set: Option<String>,
    collation: Option<String>,
) -> Result<(), String> {
    let allow_writes = {
        let configs = state.connections_config.read();
        configs
            .get(&connection_id)
            .map(|c| c.allow_writes)
            .unwrap_or(true)
    };
    crate::security::ensure_writes_allowed(allow_writes)?;
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(
        driver.capabilities().create_database,
        "create_database",
    )?;
    driver
        .create_database(&name, character_set.as_deref(), collation.as_deref())
        .await
}

#[tauri::command]
pub async fn drop_database(
    state: State<'_, AppState>,
    connection_id: Uuid,
    name: String,
) -> Result<(), String> {
    let allow_writes = {
        let configs = state.connections_config.read();
        configs
            .get(&connection_id)
            .map(|c| c.allow_writes)
            .unwrap_or(true)
    };
    crate::security::ensure_writes_allowed(allow_writes)?;
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(driver.capabilities().create_database, "drop_database")?;
    driver.drop_database(&name).await
}

#[tauri::command]
pub async fn get_tables(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
) -> Result<Vec<Table>, String> {
    println!("Fetching tables for database: '{}'", database);
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver.get_tables(&database).await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("-- List tables in {:?}", database);
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &sql,
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    let tables = result?;
    println!("  -> Found {} tables", tables.len());
    Ok(tables)
}

#[tauri::command]
pub async fn get_table_structure(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
) -> Result<Vec<ColumnStructure>, String> {
    let table = table.resolve(&database)?;
    let driver = state.get_driver(&connection_id)?;
    let sql = format!("-- Inspect columns of {:?}.{:?}", database, table);
    let t0 = std::time::Instant::now();
    let result = driver.get_table_structure(&table).await;
    let ms = t0.elapsed().as_millis() as u64;
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &sql,
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    result
}

#[tauri::command]
pub async fn get_foreign_keys(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
) -> Result<Vec<ForeignKey>, String> {
    let table = table.resolve(&database)?;
    let driver = state.get_driver(&connection_id)?;
    let sql = format!("-- Inspect foreign keys of {database:?}.{table:?}");
    let t0 = std::time::Instant::now();
    let result = driver.get_foreign_keys(&table).await;
    let ms = t0.elapsed().as_millis() as u64;
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &sql,
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    result
}

#[tauri::command]
pub async fn get_table_indexes(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
) -> Result<Vec<TableIndex>, String> {
    let table = table.resolve(&database)?;
    let driver = state.get_driver(&connection_id)?;
    let sql = format!("-- Inspect indexes of {:?}.{:?}", database, table);
    let t0 = std::time::Instant::now();
    let result = driver.get_table_indexes(&table).await;
    let ms = t0.elapsed().as_millis() as u64;
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &sql,
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    result
}

#[tauri::command]
pub async fn get_table_ddl(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
) -> Result<String, String> {
    let table = table.resolve(&database)?;
    let driver = state.get_driver(&connection_id)?;
    let sql = format!("-- Read DDL of {:?}.{:?}", database, table);
    let t0 = std::time::Instant::now();
    let result = driver.get_table_ddl(&table).await;
    let ms = t0.elapsed().as_millis() as u64;
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &sql,
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    result
}
