use super::results::compact_query_result_for_ipc;
use crate::database::driver::*;
use crate::filters::FilterSet;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn get_table_data(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
    page: u32,
    page_size: u32,
    filters: Option<FilterSet>,
    sort_column: Option<String>,
    sort_desc: Option<bool>,
    exact_count: Option<bool>,
    keyset: Option<KeysetPage>,
) -> Result<QueryResult, String> {
    let table = table.resolve(&database)?;
    if let Some(ref col) = sort_column {
        if !crate::security::is_safe_sort_column(col) {
            return Err("Invalid sort column".to_string());
        }
    }
    if let Some(ref keyset) = keyset {
        if !crate::security::is_safe_sort_column(&keyset.column) {
            return Err("Invalid keyset column".to_string());
        }
        if keyset.direction != "next" && keyset.direction != "prev" {
            return Err("Invalid keyset direction".to_string());
        }
    }

    let driver = state.get_driver(&connection_id)?;

    let t0 = std::time::Instant::now();
    let result = driver
        .get_table_data(
            &table,
            page,
            page_size,
            filters,
            sort_column,
            sort_desc,
            exact_count.unwrap_or(true),
            keyset,
        )
        .await;
    // Activity entries describe the operation, not a reconstructed SQL query.
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &format!("-- Load table {database:?}.{table:?}; page={page}, page_size={page_size}"),
        t0.elapsed().as_millis() as u64,
        result.as_ref().err().map(String::as_str),
    );
    result.map(compact_query_result_for_ipc)
}

fn connection_allows_writes(
    state: &State<'_, AppState>,
    connection_id: Uuid,
) -> Result<bool, String> {
    let configs = state.connections_config.read();
    Ok(configs
        .data()?
        .get(&connection_id)
        .map(|c| c.allow_writes)
        .unwrap_or(true))
}

#[tauri::command]
pub async fn apply_table_changes(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
    updates: Vec<RowChange>,
    deletions: Vec<RowDeletion>,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let table = table.resolve(&database)?;
    let n_updates = updates.len();
    let n_deletions = deletions.len();
    if n_updates > 0 || n_deletions > 0 {
        crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id)?)?;
    }
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(
        !disable_fk_checks || driver.capabilities().disable_foreign_key_checks,
        "Disabling foreign key checks",
    )?;
    crate::database::capabilities::require(driver.capabilities().edit_rows, "apply_table_changes")?;
    let t0 = std::time::Instant::now();
    let result = driver
        .apply_table_changes(&table, updates, deletions, disable_fk_checks)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    if n_updates > 0 {
        let sql = format!(
            "-- Update rows in {:?}.{:?} ({} rows)",
            database, table, n_updates
        );
        state.emit_query_log_context(
            Some(connection_id),
            Some(&database),
            &sql,
            ms,
            result.as_ref().err().map(|e| e.as_str()),
        );
    }
    if n_deletions > 0 {
        let sql = format!(
            "-- Delete rows from {:?}.{:?} ({} rows)",
            database, table, n_deletions
        );
        state.emit_query_log_context(
            Some(connection_id),
            Some(&database),
            &sql,
            ms,
            result.as_ref().err().map(|e| e.as_str()),
        );
    }
    result
}

#[tauri::command]
pub async fn insert_row(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
    values: Vec<TableChange>,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let table = table.resolve(&database)?;
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id)?)?;
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(
        !disable_fk_checks || driver.capabilities().disable_foreign_key_checks,
        "Disabling foreign key checks",
    )?;
    crate::database::capabilities::require(driver.capabilities().edit_rows, "insert_row")?;
    let t0 = std::time::Instant::now();
    let result = driver.insert_row(&table, values, disable_fk_checks).await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("-- Insert row into {:?}.{:?}", database, table);
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
pub async fn alter_table_column(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
    old_name: String,
    new_name: String,
    new_type: String,
) -> Result<(), String> {
    let table = table.resolve(&database)?;
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id)?)?;
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(
        driver.capabilities().alter_columns,
        "alter_table_column",
    )?;
    let t0 = std::time::Instant::now();
    let result = driver
        .alter_table_column(&table, &old_name, &new_name, &new_type)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    let logged_sql = result
        .as_ref()
        .ok()
        .filter(|sql| !sql.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            format!(
                "-- Alter column {:?}.{:?}: {:?} -> {:?} ({})",
                database, table, old_name, new_name, new_type
            )
        });
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &logged_sql,
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    result.map(|_| ())
}

#[tauri::command]
pub async fn drop_table(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let table = table.resolve(&database)?;
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id)?)?;
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(
        !disable_fk_checks || driver.capabilities().disable_foreign_key_checks,
        "Disabling foreign key checks",
    )?;
    let t0 = std::time::Instant::now();
    let result = driver.drop_table(&table, disable_fk_checks).await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("-- Drop table {:?}.{:?}", database, table);
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
pub async fn drop_tables(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    tables: Vec<TableTarget>,
    disable_fk_checks: bool,
) -> Result<(), String> {
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id)?)?;
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(
        !disable_fk_checks || driver.capabilities().disable_foreign_key_checks,
        "Disabling foreign key checks",
    )?;
    let tables = tables
        .into_iter()
        .map(|t| t.resolve(&database))
        .collect::<Result<Vec<_>, _>>()?;
    let t0 = std::time::Instant::now();
    let result = driver
        .drop_tables(&database, &tables, disable_fk_checks)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("-- Drop {} tables from {:?}", tables.len(), database);
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
pub async fn truncate_table(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: TableTarget,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let table = table.resolve(&database)?;
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id)?)?;
    let driver = state.get_driver(&connection_id)?;
    crate::database::capabilities::require(
        !disable_fk_checks || driver.capabilities().disable_foreign_key_checks,
        "Disabling foreign key checks",
    )?;
    crate::database::capabilities::require(driver.capabilities().truncate_table, "truncate_table")?;
    let t0 = std::time::Instant::now();
    let result = driver.truncate_table(&table, disable_fk_checks).await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("-- Truncate table {:?}.{:?}", database, table);
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &sql,
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    result
}
