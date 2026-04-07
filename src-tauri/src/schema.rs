use tauri::State;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use chrono::Local;
use crate::state::AppState;
use crate::driver::{ForeignKey, ImportResult, Table, TableIndex, ColumnStructure};

// --------------------------------------------------------------------------
// SQL file splitter (used by import_sql)
// --------------------------------------------------------------------------

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
            '-' if !in_single_quote && !in_double_quote && !in_backtick
                && chars.peek() == Some(&'-') =>
            {
                chars.next();
                for c in chars.by_ref() { if c == '\n' { break; } }
            }
            '/' if !in_single_quote && !in_double_quote && !in_backtick
                && chars.peek() == Some(&'*') =>
            {
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

// --------------------------------------------------------------------------
// Tauri commands
// --------------------------------------------------------------------------

#[tauri::command]
pub async fn get_databases(
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<Vec<String>, String> {
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver.get_databases().await;
    let ms = t0.elapsed().as_millis() as u64;
    state.emit_query_log("SELECT schema_name FROM information_schema.schemata ORDER BY schema_name ASC", ms, result.as_ref().err().map(|e| e.as_str()));
    result
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
    let driver = state.get_driver(&connection_id)?;
    driver.create_database(&name).await
}

#[tauri::command]
pub async fn drop_database(
    state: State<'_, AppState>,
    connection_id: Uuid,
    name: String,
) -> Result<(), String> {
    if name.is_empty() || name.contains('`') || name.contains(';') {
        return Err("Invalid database name".into());
    }
    let driver = state.get_driver(&connection_id)?;
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
    let sql = format!("SHOW FULL TABLES FROM `{}`", database);
    state.emit_query_log(&sql, ms, result.as_ref().err().map(|e| e.as_str()));
    let tables = result?;
    println!("  -> Found {} tables", tables.len());
    Ok(tables)
}

#[tauri::command]
pub async fn get_table_structure(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
) -> Result<Vec<ColumnStructure>, String> {
    let driver = state.get_driver(&connection_id)?;
    driver.get_table_structure(&database, &table).await
}

#[tauri::command]
pub async fn get_foreign_keys(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
) -> Result<Vec<ForeignKey>, String> {
    let driver = state.get_driver(&connection_id)?;
    driver.get_foreign_keys(&database, &table).await
}

#[tauri::command]
pub async fn get_table_indexes(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
) -> Result<Vec<TableIndex>, String> {
    let driver = state.get_driver(&connection_id)?;
    driver.get_table_indexes(&database, &table).await
}

#[derive(Clone, Serialize, Deserialize)]
struct Progress {
    current: usize,
    total: usize,
    status: String,
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
    use tauri::Emitter;

    let driver = state.get_driver(&connection_id)?;

    let tables_to_export = match tables {
        Some(t) => t,
        None => driver.get_base_tables(&database).await?,
    };

    let total_tables = tables_to_export.len();
    let include_structure = mode == "structure" || mode == "full";
    let include_data = mode == "data" || mode == "full";

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut out = format!(
        "-- DB Viewer Export\n-- Database: `{}`\n-- Mode: {}\n-- Generated: {}\n\
         -- --------------------------------------------------------\n\n\
         SET FOREIGN_KEY_CHECKS=0;\n\n",
        database, mode, now
    );

    let mut total_rows = 0usize;

    for (i, table) in tables_to_export.iter().enumerate() {
        let _ = window.emit("export-progress", Progress {
            current: i,
            total: total_tables,
            status: format!("Exporting table {} of {} ({})", i + 1, total_tables, table),
        });

        out.push_str(&format!(
            "-- --------------------------------------------------------\n\
             -- Table: `{}`\n\
             -- --------------------------------------------------------\n\n",
            table
        ));

        if include_structure {
            let create_sql = driver.get_table_ddl(&database, table).await?;
            out.push_str(&format!("DROP TABLE IF EXISTS `{}`;\n", table));
            out.push_str(&create_sql);
            out.push_str(";\n\n");
        }

        if include_data {
            let (columns, result_rows) = driver.get_all_rows(&database, table).await?;
            if !result_rows.is_empty() {
                let col_names = columns.iter()
                    .map(|c| format!("`{}`", c.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                for row in &result_rows {
                    if let serde_json::Value::Object(map) = row {
                        let values: Vec<String> = columns.iter().map(|c| {
                            match map.get(&c.name) {
                                Some(serde_json::Value::Null) | None => "NULL".to_string(),
                                Some(serde_json::Value::String(s)) => {
                                    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
                                }
                                Some(serde_json::Value::Bool(b)) => {
                                    if *b { "1".to_string() } else { "0".to_string() }
                                }
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

#[tauri::command]
pub async fn import_sql(
    window: tauri::Window,
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    path: String,
) -> Result<ImportResult, String> {
    use tauri::Emitter;

    let env = {
        let configs = state.connections_config.read();
        configs
            .get(&connection_id)
            .map(|c| c.environment)
            .unwrap_or(crate::connections::Environment::Local)
    };

    if env == crate::connections::Environment::Production {
        return Err("Import blocked: Production environment is READ-ONLY.".into());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let statements = split_sql_statements(&content);
    let total = statements.len();

    let _ = window.emit("import-progress", Progress {
        current: 0,
        total,
        status: format!("Executing {} statements...", total),
    });

    let driver = state.get_driver(&connection_id)?;
    let results = driver.execute_statements(&database, &statements).await;

    let executed = results.iter().filter(|r| r.is_ok()).count();
    let errors: Vec<String> = results
        .into_iter()
        .zip(statements.iter())
        .filter_map(|(r, stmt)| {
            r.err().map(|e| format!("{}: {}", &stmt[..stmt.len().min(60)], e))
        })
        .collect();

    let _ = window.emit("import-progress", Progress {
        current: total,
        total,
        status: "Import complete".to_string(),
    });

    Ok(ImportResult { executed, errors })
}
