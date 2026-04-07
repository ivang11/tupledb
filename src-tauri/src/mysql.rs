use tauri::State;
use uuid::Uuid;
use serde_json::{Value, Map};
use sqlx::{Column, Row, TypeInfo, ValueRef, MySqlPool};
use chrono::Timelike;
use async_trait::async_trait;
use crate::state::AppState;
use crate::driver::{
    ColumnInfo, ColumnStructure, DatabaseDriver, ForeignKey, QueryResult,
    RawQueryResult, RowChange, RowDeletion, Table, TableChange, TableIndex,
};
use crate::filters::FilterSet;

// --------------------------------------------------------------------------
// WKB → WKT parser (MySQL prepends a 4-byte SRID to standard WKB)
// --------------------------------------------------------------------------

fn read_u32_wkb(data: &[u8], le: bool) -> u32 {
    let b = [data[0], data[1], data[2], data[3]];
    if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
}

fn read_f64_wkb(data: &[u8], le: bool) -> f64 {
    let b = [data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]];
    if le { f64::from_le_bytes(b) } else { f64::from_be_bytes(b) }
}

fn wkb_parse(data: &[u8]) -> Option<String> {
    if data.len() < 5 { return None; }
    let le = data[0] == 1;
    let geom_type = read_u32_wkb(&data[1..5], le);
    let payload = &data[5..];
    match geom_type {
        1 => { // Point
            if payload.len() < 16 { return None; }
            let x = read_f64_wkb(&payload[0..8], le);
            let y = read_f64_wkb(&payload[8..16], le);
            Some(format!("POINT({} {})", x, y))
        }
        2 => { // LineString
            if payload.len() < 4 { return None; }
            let n = read_u32_wkb(&payload[0..4], le) as usize;
            let coords = &payload[4..];
            if coords.len() < n * 16 { return None; }
            let pts: Vec<String> = (0..n).map(|i| {
                let x = read_f64_wkb(&coords[i*16..i*16+8], le);
                let y = read_f64_wkb(&coords[i*16+8..i*16+16], le);
                format!("{} {}", x, y)
            }).collect();
            Some(format!("LINESTRING({})", pts.join(", ")))
        }
        3 => { // Polygon
            if payload.len() < 4 { return None; }
            let n_rings = read_u32_wkb(&payload[0..4], le) as usize;
            let mut offset = 4usize;
            let mut rings = Vec::new();
            for _ in 0..n_rings {
                if payload.len() < offset + 4 { return None; }
                let n_pts = read_u32_wkb(&payload[offset..offset+4], le) as usize;
                offset += 4;
                if payload.len() < offset + n_pts * 16 { return None; }
                let pts: Vec<String> = (0..n_pts).map(|i| {
                    let x = read_f64_wkb(&payload[offset+i*16..offset+i*16+8], le);
                    let y = read_f64_wkb(&payload[offset+i*16+8..offset+i*16+16], le);
                    format!("{} {}", x, y)
                }).collect();
                offset += n_pts * 16;
                rings.push(format!("({})", pts.join(", ")));
            }
            Some(format!("POLYGON({})", rings.join(", ")))
        }
        _ => None
    }
}

fn mysql_wkb_to_wkt(data: &[u8]) -> String {
    // MySQL spatial columns have a 4-byte SRID prefix before the WKB
    if data.len() > 4 {
        if let Some(wkt) = wkb_parse(&data[4..]) {
            return wkt;
        }
    }
    let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
    format!("0x{}", hex)
}

pub fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// --------------------------------------------------------------------------
// Row parser: MySqlRow → (Vec<ColumnInfo>, Vec<Value>)
// --------------------------------------------------------------------------

fn rows_to_parsed(rows: Vec<sqlx::mysql::MySqlRow>) -> (Vec<ColumnInfo>, Vec<Value>) {
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
                    match type_name.as_str() {
                        "TINYINT(1)" | "BOOLEAN" | "BOOL" => {
                            row.try_get::<i8, _>(col_name)
                                .map(|v| Value::Bool(v != 0))
                                .unwrap_or(Value::Null)
                        }
                        t if t == "BIT" || t.starts_with("BIT(") => {
                            let width: u32 = t.trim_start_matches("BIT(")
                                .trim_end_matches(')')
                                .parse()
                                .unwrap_or(1);
                            let to_bin = |n: u64| Value::String(format!("{:0>width$b}", n, width = width as usize));
                            row.try_get::<u64, _>(col_name)
                                .map(|v| to_bin(v))
                                .or_else(|_| row.try_get::<Vec<u8>, _>(col_name)
                                    .map(|b| {
                                        let n = b.iter().fold(0u64, |acc, &x| (acc << 8) | x as u64);
                                        to_bin(n)
                                    }))
                                .unwrap_or(Value::Null)
                        }
                        t if t.contains("INT") => {
                            row.try_get::<i64, _>(col_name)
                                .map(|v| Value::Number(v.into()))
                                .or_else(|_| row.try_get::<u64, _>(col_name).map(|v| Value::Number(v.into())))
                                .unwrap_or(Value::Null)
                        }
                        t if t == "DOUBLE" || t == "FLOAT" || t.starts_with("DOUBLE") || t.starts_with("FLOAT") => {
                            row.try_get::<f64, _>(col_name)
                                .ok()
                                .and_then(|v| serde_json::Number::from_f64(v))
                                .map(Value::Number)
                                .unwrap_or(Value::Null)
                        }
                        t if t.starts_with("DECIMAL") || t.starts_with("NUMERIC") || t == "NEWDECIMAL" => {
                            row.try_get::<rust_decimal::Decimal, _>(col_name)
                                .map(|d| Value::String(d.to_string()))
                                .or_else(|_| row.try_get::<String, _>(col_name).map(Value::String))
                                .unwrap_or(Value::Null)
                        }
                        "YEAR" => {
                            row.try_get::<u16, _>(col_name)
                                .map(|v| Value::Number(v.into()))
                                .or_else(|_| row.try_get::<String, _>(col_name).map(Value::String))
                                .unwrap_or(Value::Null)
                        }
                        "DATE" => {
                            row.try_get::<chrono::NaiveDate, _>(col_name)
                                .map(|d| Value::String(d.to_string()))
                                .or_else(|_| row.try_get::<String, _>(col_name).map(Value::String))
                                .unwrap_or(Value::Null)
                        }
                        t if t == "TIME" || t.starts_with("TIME(") => {
                            row.try_get::<chrono::NaiveTime, _>(col_name)
                                .map(|t| {
                                    let base = t.format("%H:%M:%S").to_string();
                                    if t.nanosecond() == 0 {
                                        base
                                    } else {
                                        let frac = format!("{:.6}", t.nanosecond() as f64 / 1_000_000_000.0);
                                        format!("{}{}", base, frac.trim_start_matches('0').trim_end_matches('0'))
                                    }
                                })
                                .map(Value::String)
                                .or_else(|_| row.try_get::<String, _>(col_name).map(Value::String))
                                .unwrap_or(Value::Null)
                        }
                        t if t == "DATETIME" || t.starts_with("DATETIME(") => {
                            row.try_get::<chrono::NaiveDateTime, _>(col_name)
                                .map(|dt| {
                                    let base = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                                    if dt.nanosecond() == 0 { base }
                                    else {
                                        let frac = format!("{:.6}", dt.nanosecond() as f64 / 1_000_000_000.0);
                                        format!("{}{}", base, frac.trim_start_matches('0').trim_end_matches('0'))
                                    }
                                })
                                .map(Value::String)
                                .or_else(|_| row.try_get::<String, _>(col_name).map(Value::String))
                                .unwrap_or(Value::Null)
                        }
                        t if t == "TIMESTAMP" || t.starts_with("TIMESTAMP(") => {
                            let s = row.try_get::<chrono::NaiveDateTime, _>(col_name)
                                .map(|dt| {
                                    let base = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                                    if dt.nanosecond() == 0 { base }
                                    else {
                                        let frac = format!("{:.6}", dt.nanosecond() as f64 / 1_000_000_000.0);
                                        format!("{}{}", base, frac.trim_start_matches('0').trim_end_matches('0'))
                                    }
                                })
                                .or_else(|_| row.try_get::<chrono::DateTime<chrono::Utc>, _>(col_name)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()))
                                .or_else(|_| row.try_get::<String, _>(col_name));
                            s.map(Value::String).unwrap_or(Value::Null)
                        }
                        t if t.contains("BLOB") || t == "BINARY" || t.starts_with("VARBINARY") => {
                            row.try_get::<Vec<u8>, _>(col_name)
                                .map(|b| {
                                    let trimmed: Vec<u8> = b.iter().copied()
                                        .rev().skip_while(|&x| x == 0).collect::<Vec<_>>()
                                        .into_iter().rev().collect();
                                    match String::from_utf8(trimmed) {
                                        Ok(s) => Value::String(s),
                                        Err(_) => {
                                            let hex: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
                                            Value::String(format!("0x{}", hex))
                                        }
                                    }
                                })
                                .unwrap_or(Value::Null)
                        }
                        t if t == "GEOMETRY" || t == "POINT" || t == "LINESTRING" || t == "POLYGON"
                          || t.starts_with("MULTI") || t == "GEOMETRYCOLLECTION" => {
                            row.try_get_unchecked::<Vec<u8>, _>(col_name)
                                .map(|b| Value::String(mysql_wkb_to_wkt(&b)))
                                .or_else(|_| row.try_get_unchecked::<String, _>(col_name).map(Value::String))
                                .unwrap_or_else(|_| Value::String(format!("<{}>", type_name)))
                        }
                        t if t == "JSON" || t.contains("JSON") => {
                            row.try_get_unchecked::<String, _>(col_name)
                                .or_else(|_| row.try_get_unchecked::<Vec<u8>, _>(col_name)
                                    .map(|b| String::from_utf8_lossy(&b).to_string()))
                                .map(Value::String)
                                .unwrap_or_else(|_| Value::String(format!("<{}>", type_name)))
                        }
                        _ => {
                            row.try_get::<String, _>(col_name).map(Value::String)
                                .or_else(|_| row.try_get::<Vec<u8>, _>(col_name)
                                    .map(|b| Value::String(String::from_utf8_lossy(&b).to_string())))
                                .unwrap_or_else(|_| Value::String(format!("<{}>", type_name)))
                        }
                    }
                }
            };
            map.insert(col_name.to_string(), value);
        }
        result_rows.push(Value::Object(map));
    }

    (columns, result_rows)
}

// --------------------------------------------------------------------------
// Row helper: try String, fallback to bytes
// --------------------------------------------------------------------------

fn get_str_lossy(row: &sqlx::mysql::MySqlRow, index: usize) -> String {
    if let Ok(s) = row.try_get::<String, _>(index) { return s; }
    if let Ok(b) = row.try_get::<Vec<u8>, _>(index) {
        return String::from_utf8_lossy(&b).to_string();
    }
    "unknown".to_string()
}

// --------------------------------------------------------------------------
// MySqlDriver
// --------------------------------------------------------------------------

pub struct MySqlDriver {
    pool: MySqlPool,
}

impl MySqlDriver {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    // --- Schema ---

    async fn get_databases(&self) -> Result<Vec<String>, String> {
        let rows = sqlx::query(
            "SELECT schema_name FROM information_schema.schemata ORDER BY schema_name ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch databases: {}", e))?;
        Ok(rows.iter().map(|row| get_str_lossy(row, 0)).collect())
    }

    async fn create_database(&self, name: &str) -> Result<(), String> {
        sqlx::query(&format!("CREATE DATABASE `{}`", name))
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to create database: {}", e))?;
        Ok(())
    }

    async fn drop_database(&self, name: &str) -> Result<(), String> {
        sqlx::query(&format!("DROP DATABASE `{}`", name))
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to drop database: {}", e))?;
        Ok(())
    }

    async fn get_tables(&self, database: &str) -> Result<Vec<Table>, String> {
        let query = format!("SHOW FULL TABLES FROM `{}`", database);
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                println!("  -> Error fetching tables: {}", e);
                format!("Failed to fetch tables: {}", e)
            })?;
        let mut tables: Vec<Table> = rows.iter().map(|row| Table {
            name: get_str_lossy(row, 0),
            table_type: get_str_lossy(row, 1),
        }).collect();
        tables.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(tables)
    }

    async fn get_table_structure(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ColumnStructure>, String> {
        let query = format!("SHOW COLUMNS FROM `{}`.`{}`", database, table);
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch structure: {}", e))?;
        Ok(rows.iter().map(|row| ColumnStructure {
            field: get_str_lossy(row, 0),
            field_type: get_str_lossy(row, 1),
            nullable: get_str_lossy(row, 2) == "YES",
            key: get_str_lossy(row, 3),
            default_value: row.try_get::<Option<String>, _>(4).ok().flatten(),
            extra: get_str_lossy(row, 5),
        }).collect())
    }

    async fn get_table_ddl(&self, database: &str, table: &str) -> Result<String, String> {
        let query = format!("SHOW CREATE TABLE `{}`.`{}`", database, table);
        let row = sqlx::query(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Failed to get DDL for {}: {}", table, e))?;
        Ok(get_str_lossy(&row, 1))
    }

    async fn get_base_tables(&self, database: &str) -> Result<Vec<String>, String> {
        let query = format!(
            "SELECT TABLE_NAME FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_TYPE = 'BASE TABLE' ORDER BY TABLE_NAME",
            database.replace('\'', "\\'")
        );
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch tables: {}", e))?;
        Ok(rows.iter().map(|r| get_str_lossy(r, 0)).collect())
    }

    async fn get_foreign_keys(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ForeignKey>, String> {
        let query = format!(
            "SELECT COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME \
             FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' AND REFERENCED_TABLE_NAME IS NOT NULL",
            database.replace('\'', "\\'"),
            table.replace('\'', "\\'")
        );
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch foreign keys: {}", e))?;
        Ok(rows.iter().map(|row| ForeignKey {
            column: get_str_lossy(row, 0),
            referenced_table: get_str_lossy(row, 1),
            referenced_column: get_str_lossy(row, 2),
        }).collect())
    }

    async fn get_table_indexes(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<TableIndex>, String> {
        let query = format!("SHOW INDEX FROM `{}`.`{}`", database, table);
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch indexes: {}", e))?;
        Ok(rows.iter().map(|row| {
            let non_unique: bool = get_str_lossy(row, 1) != "0";
            let seq: u64 = get_str_lossy(row, 3).parse().unwrap_or(1);
            TableIndex {
                key_name: get_str_lossy(row, 2),
                non_unique,
                column_name: get_str_lossy(row, 4),
                seq_in_index: seq,
                index_type: get_str_lossy(row, 10),
                nullable: get_str_lossy(row, 9) == "YES",
                comment: get_str_lossy(row, 11),
            }
        }).collect())
    }

    // --- Data ---

    async fn get_table_data(
        &self,
        database: &str,
        table: &str,
        page: u32,
        page_size: u32,
        filters: Option<FilterSet>,
        sort_column: Option<String>,
        sort_desc: Option<bool>,
    ) -> Result<QueryResult, String> {
        let (where_clause, params) = if let Some(f) = filters {
            crate::query_builder::build_where_clause(&f)
        } else {
            ("".to_string(), vec![])
        };

        let offset = page * page_size;

        let count_query = format!(
            "SELECT COUNT(*) as total FROM `{}`.`{}` {}",
            database, table, where_clause
        );
        let mut q = sqlx::query_as::<_, (i64,)>(&count_query);
        for p in &params { q = q.bind(p); }
        let (total_count,) = q
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch total count: {}", e))?;

        let order_sql = match sort_column {
            Some(ref col) => {
                let desc = sort_desc.unwrap_or(false);
                format!(" ORDER BY `{}` {}", col, if desc { "DESC" } else { "ASC" })
            }
            None => String::new(),
        };

        let data_query = format!(
            "SELECT * FROM `{}`.`{}` {}{} LIMIT {} OFFSET {}",
            database, table, where_clause, order_sql, page_size, offset
        );
        let mut q = sqlx::query(&data_query);
        for p in &params { q = q.bind(p); }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch data: {}", e))?;

        let (columns, result_rows) = rows_to_parsed(rows);
        Ok(QueryResult { columns, rows: result_rows, total_count })
    }

    async fn get_all_rows(
        &self,
        database: &str,
        table: &str,
    ) -> Result<(Vec<ColumnInfo>, Vec<Value>), String> {
        let query = format!("SELECT * FROM `{}`.`{}`", database, table);
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch data: {}", e))?;
        Ok(rows_to_parsed(rows))
    }

    async fn execute_query(
        &self,
        database: Option<&str>,
        sql: &str,
    ) -> Result<RawQueryResult, String> {
        let trimmed = sql.trim().to_uppercase();
        let is_select = trimmed.starts_with("SELECT")
            || trimmed.starts_with("SHOW")
            || trimmed.starts_with("DESCRIBE")
            || trimmed.starts_with("DESC")
            || trimmed.starts_with("EXPLAIN")
            || trimmed.starts_with("WITH");

        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;

        use sqlx::Executor;

        // USE and SHOW/DESCRIBE require the simple query protocol (not prepared).
        // Passing &str (not a prepared query) uses the simple protocol.
        if let Some(db) = database {
            if !db.is_empty() {
                let use_stmt = format!("USE `{}`", db);
                conn.execute(use_stmt.as_str())
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        if is_select {
            let rows: Vec<sqlx::mysql::MySqlRow> = conn
                .fetch_all(sql)
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
            let result: sqlx::mysql::MySqlQueryResult = conn
                .execute(sql)
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

    // --- Mutations ---

    async fn apply_table_changes(
        &self,
        database: &str,
        table: &str,
        updates: Vec<RowChange>,
        deletions: Vec<RowDeletion>,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;

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

        tx.commit().await.map_err(|e| e.to_string())
    }

    async fn insert_row(
        &self,
        database: &str,
        table: &str,
        values: Vec<TableChange>,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        if values.is_empty() {
            return Err("No values provided".to_string());
        }

        fn is_sql_expression(s: &str) -> bool {
            let upper = s.trim().to_uppercase();
            matches!(upper.as_str(),
                "NOW()" | "CURRENT_TIMESTAMP" | "CURRENT_TIMESTAMP()" |
                "CURRENT_DATE" | "CURRENT_DATE()" | "CURRENT_TIME" | "CURRENT_TIME()" |
                "UUID()" | "NULL"
            )
        }

        let columns: Vec<String> = values.iter().map(|v| format!("`{}`", v.column)).collect();
        let placeholders: Vec<String> = values.iter().map(|v| {
            if let Value::String(s) = &v.value {
                if is_sql_expression(s) { return s.trim().to_uppercase(); }
            }
            "?".to_string()
        }).collect();

        let query = format!(
            "INSERT INTO `{}`.`{}` ({}) VALUES ({})",
            database, table,
            columns.join(", "),
            placeholders.join(", ")
        );

        let mut q = sqlx::query(&query);
        for v in &values {
            if let Value::String(s) = &v.value {
                if is_sql_expression(s) { continue; }
            }
            q = match &v.value {
                Value::Null => q.bind(None::<String>),
                Value::Bool(b) => q.bind(if *b { 1i64 } else { 0i64 }),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() { q.bind(i) }
                    else if let Some(f) = n.as_f64() { q.bind(f.to_string()) }
                    else { return Err("Invalid number".to_string()); }
                }
                Value::String(s) => q.bind(s.clone()),
                _ => return Err("Unsupported value type".to_string()),
            };
        }

        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        q.execute(&mut *tx).await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())
    }

    async fn drop_table(
        &self,
        database: &str,
        table: &str,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        }
        let res = sqlx::query(&format!("DROP TABLE `{}`.`{}`", database, table))
            .execute(&mut *conn)
            .await;
        if disable_fk_checks {
            let _ = sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
                .execute(&mut *conn)
                .await;
        }
        res.map(|_| ()).map_err(|e| e.to_string())
    }

    async fn truncate_table(
        &self,
        database: &str,
        table: &str,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            let res = sqlx::query(&format!("DELETE FROM `{}`.`{}`", database, table))
                .execute(&mut *conn)
                .await;
            // Simulate TRUNCATE by resetting auto-increment
            let _ = sqlx::query(&format!(
                "ALTER TABLE `{}`.`{}` AUTO_INCREMENT = 1",
                database, table
            ))
            .execute(&mut *conn)
            .await;
            let _ = sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
                .execute(&mut *conn)
                .await;
            res.map(|_| ()).map_err(|e| e.to_string())
        } else {
            sqlx::query(&format!("TRUNCATE TABLE `{}`.`{}`", database, table))
                .execute(&mut *conn)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    }

    // --- Import ---

    async fn execute_statements(
        &self,
        database: &str,
        statements: &[String],
    ) -> Vec<Result<(), String>> {
        let mut conn = match self.pool.acquire().await {
            Ok(c) => c,
            Err(e) => return vec![Err(format!("Failed to acquire connection: {}", e))],
        };

        use sqlx::Executor;

        let use_query = format!("USE `{}`", database);
        if let Err(e) = conn.execute(use_query.as_str()).await {
            return vec![Err(format!("Failed to select database: {}", e))];
        }
        let _ = conn.execute("SET FOREIGN_KEY_CHECKS=0").await;

        let mut results = Vec::with_capacity(statements.len());
        for stmt in statements {
            results.push(
                conn.execute(stmt.as_str())
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
            );
        }

        let _ = conn.execute("SET FOREIGN_KEY_CHECKS=1").await;
        results
    }
}

// --------------------------------------------------------------------------
// Tauri commands — thin wrappers over the driver
// --------------------------------------------------------------------------

#[tauri::command]
pub async fn get_table_data(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    page: u32,
    page_size: u32,
    filters: Option<FilterSet>,
    sort_column: Option<String>,
    sort_desc: Option<bool>,
) -> Result<QueryResult, String> {
    if let Some(ref col) = sort_column {
        if !crate::security::is_safe_sort_column(col) {
            return Err("Invalid sort column".to_string());
        }
    }
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver
        .get_table_data(&database, &table, page, page_size, filters, sort_column, sort_desc)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("SELECT * FROM `{}`.`{}` LIMIT {} OFFSET {}", database, table, page_size, page * page_size);
    state.emit_query_log(&sql, ms, result.as_ref().err().map(|e| e.as_str()));
    result
}

#[tauri::command]
pub async fn execute_query(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: Option<String>,
    sql: String,
) -> Result<RawQueryResult, String> {
    let env = {
        let configs = state.connections_config.read();
        configs
            .get(&connection_id)
            .map(|c| c.environment)
            .unwrap_or(crate::connections::Environment::Local)
    };
    crate::security::is_query_safe(&sql, env)?;
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver.execute_query(database.as_deref(), &sql).await;
    let ms = t0.elapsed().as_millis() as u64;
    state.emit_query_log(&sql, ms, result.as_ref().err().map(|e| e.as_str()));
    result
}

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
    table: String,
    format: String,
    path: String,
) -> Result<usize, String> {
    use tauri::Emitter;

    let _ = window.emit("export-progress", ExportProgress {
        current: 0,
        total: 100,
        status: format!("Fetching data from {}...", table),
    });

    let driver = state.get_driver(&connection_id)?;
    let (columns, result_rows) = driver.get_all_rows(&database, &table).await?;
    let row_count = result_rows.len();

    let _ = window.emit("export-progress", ExportProgress {
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
                    let _ = window.emit("export-progress", ExportProgress {
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
        "json" => serde_json::to_string_pretty(&result_rows).map_err(|e| e.to_string())?,
        "sql" => {
            let col_names = columns.iter()
                .map(|c| format!("`{}`", c.name))
                .collect::<Vec<_>>()
                .join(", ");
            let mut out = format!("-- Export of `{}`.`{}`\n\n", database, table);
            for (i, row) in result_rows.iter().enumerate() {
                if i % 1000 == 0 && row_count > 0 {
                    let _ = window.emit("export-progress", ExportProgress {
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

    let _ = window.emit("export-progress", ExportProgress {
        current: 90,
        total: 100,
        status: "Saving file...".to_string(),
    });

    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {}", e))?;

    let _ = window.emit("export-progress", ExportProgress {
        current: 100,
        total: 100,
        status: "Export complete".to_string(),
    });

    Ok(row_count)
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
    let n_updates = updates.len();
    let n_deletions = deletions.len();
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver
        .apply_table_changes(&database, &table, updates, deletions, disable_fk_checks)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    if n_updates > 0 {
        let sql = format!("UPDATE `{}`.`{}` SET ... ({} row(s))", database, table, n_updates);
        state.emit_query_log(&sql, ms, result.as_ref().err().map(|e| e.as_str()));
    }
    if n_deletions > 0 {
        let sql = format!("DELETE FROM `{}`.`{}` ({} row(s))", database, table, n_deletions);
        state.emit_query_log(&sql, ms, result.as_ref().err().map(|e| e.as_str()));
    }
    result
}

#[tauri::command]
pub async fn insert_row(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    values: Vec<TableChange>,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver.insert_row(&database, &table, values, disable_fk_checks).await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("INSERT INTO `{}`.`{}`", database, table);
    state.emit_query_log(&sql, ms, result.as_ref().err().map(|e| e.as_str()));
    result
}

#[tauri::command]
pub async fn drop_table(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver.drop_table(&database, &table, disable_fk_checks).await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("DROP TABLE `{}`.`{}`", database, table);
    state.emit_query_log(&sql, ms, result.as_ref().err().map(|e| e.as_str()));
    result
}

#[tauri::command]
pub async fn truncate_table(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: String,
    table: String,
    disable_fk_checks: bool,
) -> Result<(), String> {
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver.truncate_table(&database, &table, disable_fk_checks).await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("TRUNCATE TABLE `{}`.`{}`", database, table);
    state.emit_query_log(&sql, ms, result.as_ref().err().map(|e| e.as_str()));
    result
}
