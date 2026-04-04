use tauri::State;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use crate::state::AppState;
use chrono::Local;

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub executed: usize,
    pub errors: Vec<String>,
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => {
                escaped = true;
                current.push(ch);
            }
            '\'' if !in_double_quote && !in_backtick => {
                in_single_quote = !in_single_quote;
                current.push(ch);
            }
            '"' if !in_single_quote && !in_backtick => {
                in_double_quote = !in_double_quote;
                current.push(ch);
            }
            '`' if !in_single_quote && !in_double_quote => {
                in_backtick = !in_backtick;
                current.push(ch);
            }
            '-' if !in_single_quote && !in_double_quote && !in_backtick && chars.peek() == Some(&'-') => {
                chars.next();
                for c in chars.by_ref() { if c == '\n' { break; } }
            }
            '/' if !in_single_quote && !in_double_quote && !in_backtick && chars.peek() == Some(&'*') => {
                chars.next();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'/') => { chars.next(); break; }
                        None => break,
                        _ => {}
                    }
                }
            }
            ';' if !in_single_quote && !in_double_quote && !in_backtick => {
                let stmt = current.trim().to_string();
                if !stmt.is_empty() { statements.push(stmt); }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let stmt = current.trim().to_string();
    if !stmt.is_empty() { statements.push(stmt); }
    statements
}

#[tauri::command]
pub async fn import_sql(
    window: tauri::Window,
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    path: String,
) -> Result<ImportResult, String> {
    let env = {
        let configs = state.connections_config.read();
        configs.get(&connection_id).map(|c| c.environment).unwrap_or(crate::connections::Environment::Local)
    };

    if env == crate::connections::Environment::Production {
        return Err("Import blocked: Production environment is READ-ONLY.".into());
    }

    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let mut conn = pool.acquire().await
        .map_err(|e| format!("Failed to acquire connection: {}", e))?;

    use sqlx::Executor;
    use tauri::Emitter;

    #[derive(Clone, Serialize)]
    struct Progress {
        current: usize,
        total: usize,
        status: String,
    }

    // Usar .execute(String) directamente para evitar problemas de lifetimes con raw_sql
    let use_query = format!("USE `{}`", database);
    conn.execute(use_query.as_str())
        .await
        .map_err(|e| format!("Failed to select database: {}", e))?;

    conn.execute("SET FOREIGN_KEY_CHECKS=0")
        .await
        .map_err(|e| format!("Failed to disable FK checks: {}", e))?;

    let statements = split_sql_statements(&content);
    let total = statements.len();
    let mut executed = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for (i, stmt) in statements.iter().enumerate() {
        if i % 10 == 0 || i == total - 1 {
            let _ = window.emit("import-progress", Progress {
                current: i + 1,
                total,
                status: format!("Executing statement {} of {}", i + 1, total),
            });
        }

        match conn.execute(stmt.as_str()).await {
            Ok(_) => executed += 1,
            Err(e) => errors.push(format!("{}: {}", &stmt[..stmt.len().min(60)], e)),
        }
    }

    let _ = conn.execute("SET FOREIGN_KEY_CHECKS=1").await;

    Ok(ImportResult { executed, errors })
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Table {
    pub name: String,
    pub table_type: String,
}

/// Intenta obtener un String de una columna, manejando conversiones de bytes si es necesario
fn get_string_lossy(row: &sqlx::mysql::MySqlRow, index: usize) -> String {
    // 1. Intentar como String normal
    if let Ok(s) = row.try_get::<String, _>(index) {
        return s;
    }
    
    // 2. Si falla, intentar como bytes (Vec<u8>)
    if let Ok(b) = row.try_get::<Vec<u8>, _>(index) {
        return String::from_utf8_lossy(&b).to_string();
    }

    "unknown".to_string()
}

#[tauri::command]
pub async fn create_database(
    state: State<'_, AppState>,
    connection_id: Uuid,
    name: String,
) -> Result<(), String> {
    if name.is_empty() || name.contains('`') || name.contains(';') {
        return Err("Invalid database name".into());
    }
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };
    sqlx::query(&format!("CREATE DATABASE `{}`", name))
        .execute(&pool)
        .await
        .map_err(|e| format!("Failed to create database: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn get_databases(state: State<'_, AppState>, connection_id: Uuid) -> Result<Vec<String>, String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let rows = sqlx::query("SELECT schema_name FROM information_schema.schemata ORDER BY schema_name ASC")
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to fetch databases: {}", e))?;

    Ok(rows.iter().map(|row| get_string_lossy(row, 0)).collect())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColumnStructure {
    pub field: String,
    pub field_type: String,
    pub nullable: bool,
    pub key: String,
    pub default_value: Option<String>,
    pub extra: String,
}

#[tauri::command]
pub async fn get_table_structure(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
) -> Result<Vec<ColumnStructure>, String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let query = format!("SHOW COLUMNS FROM `{}`.`{}`", database, table);
    let rows = sqlx::query(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to fetch structure: {}", e))?;

    let columns = rows.iter().map(|row| ColumnStructure {
        field: get_string_lossy(row, 0),
        field_type: get_string_lossy(row, 1),
        nullable: get_string_lossy(row, 2) == "YES",
        key: get_string_lossy(row, 3),
        default_value: row.try_get::<Option<String>, _>(4).ok().flatten(),
        extra: get_string_lossy(row, 5),
    }).collect();

    Ok(columns)
}

#[tauri::command]
pub async fn export_database(
    window: tauri::Window,
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    mode: String, // "structure", "data", "full"
    path: String,
    tables: Option<Vec<String>>,
) -> Result<usize, String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    use tauri::Emitter;

    #[derive(Clone, serde::Serialize)]
    struct Progress {
        current: usize,
        total: usize,
        status: String,
    }

    let tables_to_export = if let Some(t) = tables {
        t
    } else {
        // Get all base tables (skip views) if none provided
        let tables_query = format!(
            "SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = '{}' AND TABLE_TYPE = 'BASE TABLE' ORDER BY TABLE_NAME",
            database.replace('\'', "\\'")
        );
        let table_rows = sqlx::query(&tables_query)
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("Failed to fetch tables: {}", e))?;

        table_rows.iter()
            .map(|r| get_string_lossy(r, 0))
            .collect()
    };

    let total_tables = tables_to_export.len();
    let include_structure = mode == "structure" || mode == "full";
    let include_data = mode == "data" || mode == "full";

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut out = format!(
        "-- DB Viewer Export\n-- Database: `{}`\n-- Mode: {}\n-- Generated: {}\n-- --------------------------------------------------------\n\nSET FOREIGN_KEY_CHECKS=0;\n\n",
        database, mode, now
    );

    let mut total_rows = 0usize;

    for (i, table) in tables_to_export.iter().enumerate() {
        let _ = window.emit("export-progress", Progress {
            current: i,
            total: total_tables,
            status: format!("Exporting table {} of {} ({})", i + 1, total_tables, table),
        });

        out.push_str(&format!("-- --------------------------------------------------------\n-- Table: `{}`\n-- --------------------------------------------------------\n\n", table));

        if include_structure {
            let ddl_query = format!("SHOW CREATE TABLE `{}`.`{}`", database, table);
            let ddl_row = sqlx::query(&ddl_query)
                .fetch_one(&pool)
                .await
                .map_err(|e| format!("Failed to get DDL for {}: {}", table, e))?;

            let create_sql = get_string_lossy(&ddl_row, 1);
            out.push_str(&format!("DROP TABLE IF EXISTS `{}`;\n", table));
            out.push_str(&create_sql);
            out.push_str(";\n\n");
        }

        if include_data {
            let data_query = format!("SELECT * FROM `{}`.`{}`", database, table);
            let rows = sqlx::query(&data_query)
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Failed to fetch data for {}: {}", table, e))?;

            if !rows.is_empty() {
                let (columns, result_rows) = crate::mysql::rows_to_parsed(rows);
                let col_names = columns.iter()
                    .map(|c| format!("`{}`", c.name))
                    .collect::<Vec<_>>()
                    .join(", ");

                for row in &result_rows {
                    if let serde_json::Value::Object(map) = row {
                        let values: Vec<String> = columns.iter().map(|c| {
                            match map.get(&c.name) {
                                Some(serde_json::Value::Null) | None => "NULL".to_string(),
                                Some(serde_json::Value::String(s)) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
                                Some(serde_json::Value::Bool(b)) => if *b { "1".to_string() } else { "0".to_string() },
                                Some(v) => v.to_string(),
                            }
                        }).collect();
                        out.push_str(&format!(
                            "INSERT INTO `{}` ({}) VALUES ({});\n",
                            table, col_names, values.join(", ")
                        ));
                        total_rows += 1;
                    }
                }
                out.push('\n');
            }
        }
    }

    out.push_str("SET FOREIGN_KEY_CHECKS=1;\n");
    std::fs::write(&path, out).map_err(|e| format!("Failed to write file: {}", e))?;

    let _ = window.emit("export-progress", Progress {
        current: total_tables,
        total: total_tables,
        status: "Export complete".to_string(),
    });

    Ok(total_rows)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ForeignKey {
    pub column: String,
    pub referenced_table: String,
    pub referenced_column: String,
}

#[tauri::command]
pub async fn get_foreign_keys(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
) -> Result<Vec<ForeignKey>, String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let query = format!(
        "SELECT COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME \
         FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE \
         WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' AND REFERENCED_TABLE_NAME IS NOT NULL",
        database.replace('\'', "\\'"),
        table.replace('\'', "\\'")
    );

    let rows = sqlx::query(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to fetch foreign keys: {}", e))?;

    Ok(rows.iter().map(|row| ForeignKey {
        column: get_string_lossy(row, 0),
        referenced_table: get_string_lossy(row, 1),
        referenced_column: get_string_lossy(row, 2),
    }).collect())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TableIndex {
    pub key_name: String,
    pub non_unique: bool,
    pub column_name: String,
    pub seq_in_index: u64,
    pub index_type: String,
    pub nullable: bool,
    pub comment: String,
}

#[tauri::command]
pub async fn get_table_indexes(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
) -> Result<Vec<TableIndex>, String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let query = format!("SHOW INDEX FROM `{}`.`{}`", database, table);
    let rows = sqlx::query(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to fetch indexes: {}", e))?;

    Ok(rows.iter().map(|row| {
        let non_unique_str = get_string_lossy(row, 1);
        let non_unique: bool = non_unique_str != "0";
        let seq: u64 = get_string_lossy(row, 3).parse().unwrap_or(1);
        let nullable = get_string_lossy(row, 9) == "YES";
        TableIndex {
            key_name: get_string_lossy(row, 2),
            non_unique,
            column_name: get_string_lossy(row, 4),
            seq_in_index: seq,
            index_type: get_string_lossy(row, 10),
            nullable,
            comment: get_string_lossy(row, 11),
        }
    }).collect())
}

#[tauri::command]
pub async fn get_tables(state: State<'_, AppState>, connection_id: Uuid, database: String) -> Result<Vec<Table>, String> {
    println!("Fetching tables for database: '{}'", database);
    
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    // Usamos SHOW FULL TABLES escapando el nombre de la DB. 
    // Esto es más fiable en algunos servidores que filtrar information_schema.
    let query = format!("SHOW FULL TABLES FROM `{}`", database);
    
    let rows = sqlx::query(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            println!("  -> Error fetching tables: {}", e);
            format!("Failed to fetch tables: {}", e)
        })?;

    println!("  -> Found {} tables", rows.len());

    let mut tables: Vec<Table> = rows.iter().map(|row| {
        Table {
            name: get_string_lossy(row, 0),
            // El segundo campo en SHOW FULL TABLES indica si es BASE TABLE o VIEW
            table_type: get_string_lossy(row, 1),
        }
    }).collect();

    // Ordenar alfabéticamente
    tables.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(tables)
}
