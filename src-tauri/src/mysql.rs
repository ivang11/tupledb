use tauri::State;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{Value, Map};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use crate::state::AppState;

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn rows_to_parsed(rows: Vec<sqlx::mysql::MySqlRow>) -> (Vec<ColumnInfo>, Vec<Value>) {
    let mut columns = Vec::new();
    let mut result_rows = Vec::new();

    if let Some(first_row) = rows.first() {
        for col in first_row.columns() {
            columns.push(ColumnInfo {
                name: col.name().to_string(),
                type_name: col.type_info().name().to_string(),
            });
        }
    }

    for row in rows {
        let mut map = Map::new();
        for col in row.columns() {
            let col_name = col.name();
            let type_name = col.type_info().name().to_uppercase();
            let value: Value = match row.try_get_raw(col_name) {
                Ok(raw) if raw.is_null() => Value::Null,
                _ => {
                    if type_name.contains("TINYINT(1)") || type_name == "BOOLEAN" || type_name == "BOOL" {
                        row.try_get::<i8, _>(col_name).map(|v| Value::Bool(v != 0)).unwrap_or(Value::Null)
                    } else if type_name.contains("INT") {
                        row.try_get::<i64, _>(col_name).map(|v| Value::Number(v.into()))
                            .or_else(|_| row.try_get::<u64, _>(col_name).map(|v| Value::Number(v.into())))
                            .unwrap_or(Value::Null)
                    } else if type_name.contains("DECIMAL") || type_name.contains("DOUBLE") || type_name.contains("FLOAT") {
                        row.try_get::<f64, _>(col_name)
                            .and_then(|v| serde_json::Number::from_f64(v).ok_or(sqlx::Error::RowNotFound))
                            .map(Value::Number)
                            .unwrap_or(Value::Null)
                    } else if type_name.contains("DATE") || type_name.contains("TIME") || type_name.contains("TIMESTAMP") {
                        if let Ok(dt) = row.try_get::<chrono::NaiveDateTime, _>(col_name) {
                            Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        } else if let Ok(dt) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(col_name) {
                            Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        } else if let Ok(d) = row.try_get::<chrono::NaiveDate, _>(col_name) {
                            Value::String(d.to_string())
                        } else {
                            row.try_get::<String, _>(col_name).map(Value::String)
                                .or_else(|_| row.try_get::<Vec<u8>, _>(col_name).map(|b| Value::String(String::from_utf8_lossy(&b).to_string())))
                                .unwrap_or_else(|_| Value::String("Invalid Date".into()))
                        }
                    } else {
                        row.try_get::<String, _>(col_name).map(Value::String)
                            .or_else(|_| row.try_get::<Vec<u8>, _>(col_name).map(|b| Value::String(String::from_utf8_lossy(&b).to_string())))
                            .unwrap_or(Value::Null)
                    }
                }
            };
            map.insert(col_name.to_string(), value);
        }
        result_rows.push(Value::Object(map));
    }

    (columns, result_rows)
}

#[tauri::command]
pub async fn export_table(
    window: tauri::Window,
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    format: String,
    path: String,
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

    let _ = window.emit("export-progress", Progress {
        current: 0,
        total: 100,
        status: format!("Fetching data from {}...", table),
    });

    let query = format!("SELECT * FROM `{}`.`{}`", database, table);
    let rows = sqlx::query(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to fetch data: {}", e))?;

    let row_count = rows.len();

    let _ = window.emit("export-progress", Progress {
        current: 30,
        total: 100,
        status: format!("Parsing {} rows...", row_count),
    });

    let (columns, result_rows) = rows_to_parsed(rows);

    let _ = window.emit("export-progress", Progress {
        current: 60,
        total: 100,
        status: format!("Formatting as {}...", format),
    });

    let content = match format.as_str() {
        "csv" => {
            let mut out = String::new();
            let header: Vec<String> = columns.iter().map(|c| escape_csv(&c.name)).collect();
            out.push_str(&header.join(","));
            out.push('\n');
            for (i, row) in result_rows.iter().enumerate() {
                if i % 1000 == 0 && row_count > 0 {
                    let _ = window.emit("export-progress", Progress {
                        current: 60 + ((i as f32 / row_count as f32) * 30.0) as usize,
                        total: 100,
                        status: format!("Formatting row {} of {}...", i, row_count),
                    });
                }
                if let Value::Object(map) = row {
                    let values: Vec<String> = columns.iter().map(|c| {
                        match map.get(&c.name) {
                            Some(Value::Null) | None => String::new(),
                            Some(Value::String(s)) => escape_csv(s),
                            Some(Value::Bool(b)) => b.to_string(),
                            Some(v) => v.to_string(),
                        }
                    }).collect();
                    out.push_str(&values.join(","));
                    out.push('\n');
                }
            }
            out
        }
        "json" => serde_json::to_string_pretty(&result_rows)
            .map_err(|e| e.to_string())?,
        "sql" => {
            let col_names = columns.iter()
                .map(|c| format!("`{}`", c.name))
                .collect::<Vec<_>>()
                .join(", ");
            let mut out = format!("-- Export of `{}`.`{}`\n\n", database, table);
            for (i, row) in result_rows.iter().enumerate() {
                if i % 1000 == 0 && row_count > 0 {
                    let _ = window.emit("export-progress", Progress {
                        current: 60 + ((i as f32 / row_count as f32) * 30.0) as usize,
                        total: 100,
                        status: format!("Formatting row {} of {}...", i, row_count),
                    });
                }
                if let Value::Object(map) = row {
                    let values: Vec<String> = columns.iter().map(|c| {
                        match map.get(&c.name) {
                            Some(Value::Null) | None => "NULL".to_string(),
                            Some(Value::String(s)) => format!("'{}'", s.replace('\'', "\\'")),
                            Some(Value::Bool(b)) => if *b { "1".to_string() } else { "0".to_string() },
                            Some(v) => v.to_string(),
                        }
                    }).collect();
                    out.push_str(&format!(
                        "INSERT INTO `{}` ({}) VALUES ({});\n",
                        table, col_names, values.join(", ")
                    ));
                }
            }
            out
        }
        _ => return Err(format!("Unknown format: {}", format)),
    };

    let _ = window.emit("export-progress", Progress {
        current: 90,
        total: 100,
        status: "Saving file...".to_string(),
    });

    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {}", e))?;

    let _ = window.emit("export-progress", Progress {
        current: 100,
        total: 100,
        status: "Export complete".to_string(),
    });

    Ok(row_count)
}

#[derive(Debug, Deserialize)]
pub struct TableChange {
    pub column: String,
    pub value: Value,
}

#[derive(Debug, Deserialize)]
pub struct RowChange {
    pub pk_column: String,
    pub pk_value: Value,
    pub changes: Vec<TableChange>,
}

#[derive(Debug, Deserialize)]
pub struct RowDeletion {
    pub pk_column: String,
    pub pk_value: Value,
}

#[tauri::command]
pub async fn apply_table_changes(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    updates: Vec<RowChange>,
    deletions: Vec<RowDeletion>,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    if disable_fk_checks {
        sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    for update in updates {
        for change in update.changes {
            let query = format!(
                "UPDATE `{}`.`{}` SET `{}` = ? WHERE `{}` = ?",
                database, table, change.column, update.pk_column
            );
            
            let mut q = sqlx::query(&query);
            
            // Bind value
            q = match change.value {
                Value::Null => q.bind(None::<String>),
                Value::Bool(b) => q.bind(if b { 1 } else { 0 }),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() { q.bind(i) }
                    else if let Some(f) = n.as_f64() { q.bind(f) }
                    else { return Err("Invalid number".to_string()); }
                }
                Value::String(s) => q.bind(s),
                _ => return Err("Unsupported value type".to_string()),
            };

            // Bind PK value
            q = match update.pk_value.clone() {
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() { q.bind(i) }
                    else { return Err("Invalid PK number".to_string()); }
                }
                Value::String(s) => q.bind(s),
                _ => return Err("Unsupported PK type".to_string()),
            };

            q.execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
    }

    for deletion in deletions {
        let query = format!(
            "DELETE FROM `{}`.`{}` WHERE `{}` = ?",
            database, table, deletion.pk_column
        );
        
        let mut q = sqlx::query(&query);
        q = match deletion.pk_value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() { q.bind(i) }
                else { return Err("Invalid PK number".to_string()); }
            }
            Value::String(s) => q.bind(s),
            _ => return Err("Unsupported PK type".to_string()),
        };

        q.execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    if disable_fk_checks {
        sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn drop_table(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;

    if disable_fk_checks {
        sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }

    let query = format!("DROP TABLE `{}`.`{}`", database, table);
    let res = sqlx::query(&query)
        .execute(&mut *conn)
        .await;

    if disable_fk_checks {
        let _ = sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
            .execute(&mut *conn)
            .await;
    }

    res.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn truncate_table(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;

    if disable_fk_checks {
        // For tables with FKs, DELETE is more reliable than TRUNCATE even with checks off
        sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            
        let delete_query = format!("DELETE FROM `{}`.`{}`", database, table);
        let res = sqlx::query(&delete_query)
            .execute(&mut *conn)
            .await;
            
        // Reset auto increment to simulate truncate behavior
        let _ = sqlx::query(&format!("ALTER TABLE `{}`.`{}` AUTO_INCREMENT = 1", database, table))
            .execute(&mut *conn)
            .await;

        let _ = sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
            .execute(&mut *conn)
            .await;
            
        res.map(|_| ()).map_err(|e| e.to_string())
    } else {
        let query = format!("TRUNCATE TABLE `{}`.`{}`", database, table);
        sqlx::query(&query)
            .execute(&mut *conn)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawQueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Value>,
    pub rows_affected: u64,
    pub is_select: bool,
}

#[tauri::command]
pub async fn execute_query(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: Option<String>,
    sql: String,
) -> Result<RawQueryResult, String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let env = {
        let configs = state.connections_config.read();
        configs.get(&connection_id).map(|c| c.environment).unwrap_or(crate::connections::Environment::Local)
    };

    crate::security::is_query_safe(&sql, env)?;

    let trimmed = sql.trim().to_uppercase();
    let is_select = trimmed.starts_with("SELECT")
        || trimmed.starts_with("SHOW")
        || trimmed.starts_with("DESCRIBE")
        || trimmed.starts_with("DESC")
        || trimmed.starts_with("EXPLAIN")
        || trimmed.starts_with("WITH");

    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;

    use sqlx::Executor;

    // USE and SHOW/DESCRIBE are not supported in prepared statement protocol.
    // Passing &str to conn.execute() / conn.fetch_all() uses the simple query protocol.
    if let Some(ref db) = database {
        if !db.is_empty() {
            let use_stmt = format!("USE `{}`", db);
            conn.execute(use_stmt.as_str())
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if is_select {
        let rows: Vec<sqlx::mysql::MySqlRow> = conn.fetch_all(sql.as_str())
            .await
            .map_err(|e| format!("Query error: {}", e))?;

        let row_count = rows.len() as u64;
        let (columns, result_rows) = rows_to_parsed(rows);

        Ok(RawQueryResult {
            columns,
            rows: result_rows,
            rows_affected: row_count,
            is_select: true,
        })
    } else {
        let result: sqlx::mysql::MySqlQueryResult = conn.execute(sql.as_str())
            .await
            .map_err(|e| format!("Query error: {}", e))?;

        Ok(RawQueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: result.rows_affected(),
            is_select: false,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Value>,
    pub total_count: i64,
}

#[tauri::command]
pub async fn get_table_data(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    page: u32,
    page_size: u32,
    filters: Option<crate::filters::FilterSet>,
) -> Result<QueryResult, String> {
    let pool = {
        let sessions = state.active_sessions.read();
        sessions.get(&connection_id).ok_or("No active session found")?.pool.clone()
    };

    let (where_clause, params) = if let Some(f) = filters {
        crate::query_builder::build_where_clause(&f)
    } else {
        ("".to_string(), vec![])
    };

    let offset = page * page_size;

    let env = {
        let configs = state.connections_config.read();
        configs.get(&connection_id).map(|c| c.environment).unwrap_or(crate::connections::Environment::Local)
    };
    
    let count_query = format!("SELECT COUNT(*) as total FROM `{}`.`{}` {}", database, table, where_clause);
    crate::security::is_query_safe(&count_query, env)?;
    
    let mut q = sqlx::query_as::<_, (i64,)>(&count_query);
    for p in &params {
        q = q.bind(p);
    }
    
    let (total_count,) = q
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("Failed to fetch total count: {}", e))?;

    let data_query = format!(
        "SELECT * FROM `{}`.`{}` {} LIMIT {} OFFSET {}",
        database, table, where_clause, page_size, offset
    );
    
    let mut q = sqlx::query(&data_query);
    for p in &params {
        q = q.bind(p);
    }

    let rows = q
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to fetch data: {}", e))?;

    let (columns, result_rows) = rows_to_parsed(rows);

    Ok(QueryResult {
        columns,
        rows: result_rows,
        total_count,
    })
}
