use tauri::State;
use uuid::Uuid;
use serde_json::{Value, Map};
use sqlx::{Column, Row, TypeInfo, ValueRef, MySqlPool};
use chrono::Timelike;
use async_trait::async_trait;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
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

fn parse_mysql_row(row: &sqlx::mysql::MySqlRow) -> Value {
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
    Value::Object(map)
}

fn rows_to_parsed(rows: Vec<sqlx::mysql::MySqlRow>) -> (Vec<ColumnInfo>, Vec<Value>) {
    let mut columns = Vec::new();
    if let Some(first_row) = rows.first() {
        for col in first_row.columns() {
            columns.push(ColumnInfo {
                name: col.name().to_string(),
                type_name: col.type_info().name().to_string(),
            });
        }
    }
    let result_rows = rows.iter().map(parse_mysql_row).collect();
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
    running_queries: Arc<RwLock<HashMap<String, u64>>>,
    /// True when the server has ONLY_FULL_GROUP_BY enabled (MySQL 5.7+).
    /// Used to disable it for the session in read-only export queries so that
    /// VIEWs created without strict mode can still be read.
    no_group_by_check: bool,
}

impl MySqlDriver {
    pub fn new(pool: MySqlPool, no_group_by_check: bool) -> Self {
        Self {
            pool,
            running_queries: Arc::new(RwLock::new(HashMap::new())),
            no_group_by_check,
        }
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

    async fn get_primary_key_columns(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<String>, String> {
        let query = format!(
            "SELECT column_name FROM information_schema.statistics \
             WHERE table_schema='{}' AND table_name='{}' AND index_name='PRIMARY' \
             ORDER BY seq_in_index ASC",
            database.replace('\'', "\\'"),
            table.replace('\'', "\\'")
        );
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch primary key: {}", e))?;
        Ok(rows.iter().map(|r| get_str_lossy(r, 0)).collect())
    }

    async fn get_estimated_row_count(
        &self,
        database: &str,
        table: &str,
    ) -> Result<i64, String> {
        let query = format!(
            "SELECT table_rows FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA='{}' AND TABLE_NAME='{}'",
            database.replace('\'', "\\'"),
            table.replace('\'', "\\'")
        );
        let row = sqlx::query(&query)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch estimated count: {}", e))?;
        Ok(row.map(|r| get_str_lossy(&r, 0).parse().unwrap_or(0)).unwrap_or(0))
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
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if self.no_group_by_check {
            sqlx::query("SET SESSION sql_mode=(SELECT REPLACE(@@SESSION.sql_mode,'ONLY_FULL_GROUP_BY',''))")
                .execute(&mut *conn)
                .await
                .map_err(|e| format!("Failed to set sql_mode: {}", e))?;
        }
        let mut stream = sqlx::query(&query).fetch(&mut *conn);
        let mut columns: Vec<ColumnInfo> = Vec::new();
        let mut result_rows: Vec<Value> = Vec::new();
        while let Some(row_result) = stream.next().await {
            let row = row_result.map_err(|e| format!("Failed to fetch data: {}", e))?;
            if columns.is_empty() {
                for col in row.columns() {
                    columns.push(ColumnInfo {
                        name: col.name().to_string(),
                        type_name: col.type_info().name().to_string(),
                    });
                }
            }
            result_rows.push(parse_mysql_row(&row));
        }
        Ok((columns, result_rows))
    }

    async fn stream_all_rows(
        &self,
        database: &str,
        table: &str,
        tx: tokio::sync::mpsc::Sender<(Option<Vec<ColumnInfo>>, Value)>,
    ) -> Result<(), String> {
        let query = format!("SELECT * FROM `{}`.`{}`", database, table);
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if self.no_group_by_check {
            sqlx::query("SET SESSION sql_mode=(SELECT REPLACE(@@SESSION.sql_mode,'ONLY_FULL_GROUP_BY',''))")
                .execute(&mut *conn)
                .await
                .map_err(|e| format!("Failed to set sql_mode: {}", e))?;
        }
        let mut stream = sqlx::query(&query).fetch(&mut *conn);
        let mut columns: Vec<ColumnInfo> = Vec::new();
        while let Some(row_result) = stream.next().await {
            let row = row_result.map_err(|e| format!("Failed to stream data: {}", e))?;
            let col_opt = if columns.is_empty() {
                for col in row.columns() {
                    columns.push(ColumnInfo {
                        name: col.name().to_string(),
                        type_name: col.type_info().name().to_string(),
                    });
                }
                Some(columns.clone())
            } else {
                None
            };
            if tx.send((col_opt, parse_mysql_row(&row))).await.is_err() {
                break; // receiver dropped (export cancelled)
            }
        }
        Ok(())
    }

    async fn execute_query(
        &self,
        database: Option<&str>,
        sql: &str,
        query_id: Option<&str>,
        on_progress: Option<Arc<dyn Fn(u64) + Send + Sync>>,
        on_chunk: Option<Arc<dyn Fn(Option<Vec<ColumnInfo>>, Vec<serde_json::Value>) + Send + Sync>>,
    ) -> Result<RawQueryResult, String> {
        const CHUNK_SIZE: usize = 500;

        let trimmed = sql.trim().to_uppercase();
        let is_select = trimmed.starts_with("SELECT")
            || trimmed.starts_with("SHOW")
            || trimmed.starts_with("DESCRIBE")
            || trimmed.starts_with("DESC")
            || trimmed.starts_with("EXPLAIN")
            || trimmed.starts_with("WITH");

        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;

        use sqlx::Executor;

        // Register the MySQL thread id so cancel_query can KILL it.
        if let Some(qid) = query_id {
            let thread_id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            self.running_queries.write().insert(qid.to_string(), thread_id);
        }

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

        let result = if is_select {
            let mut stream = conn.fetch(sql);
            let mut columns: Vec<ColumnInfo> = Vec::new();
            let mut row_count: u64 = 0;

            if on_chunk.is_some() {
                // ── Streaming mode: flush rows in chunks ──────────────────────
                let mut chunk_buf: Vec<Value> = Vec::with_capacity(CHUNK_SIZE);
                let mut first_chunk = true;

                while let Some(row_result) = stream.next().await {
                    let row = row_result.map_err(|e| format!("Query error: {}", e))?;
                    if columns.is_empty() {
                        for col in row.columns() {
                            columns.push(ColumnInfo {
                                name: col.name().to_string(),
                                type_name: col.type_info().name().to_string(),
                            });
                        }
                    }
                    chunk_buf.push(parse_mysql_row(&row));
                    row_count += 1;

                    if chunk_buf.len() >= CHUNK_SIZE {
                        if let Some(ref cb) = on_chunk {
                            let cols = if first_chunk { Some(columns.clone()) } else { None };
                            cb(cols, std::mem::replace(&mut chunk_buf, Vec::with_capacity(CHUNK_SIZE)));
                            first_chunk = false;
                        }
                        if let Some(ref cb) = on_progress {
                            cb(row_count);
                        }
                    }
                }
                // Flush remaining rows
                if !chunk_buf.is_empty() {
                    if let Some(ref cb) = on_chunk {
                        let cols = if first_chunk { Some(columns.clone()) } else { None };
                        cb(cols, chunk_buf);
                    }
                }
                if let Some(ref cb) = on_progress {
                    cb(row_count);
                }
                Ok(RawQueryResult {
                    columns,
                    rows: vec![], // rows were streamed via on_chunk
                    rows_affected: row_count,
                    is_select: true,
                })
            } else {
                // ── Buffered mode (legacy): accumulate all rows ───────────────
                let mut result_rows: Vec<Value> = Vec::new();
                while let Some(row_result) = stream.next().await {
                    let row = row_result.map_err(|e| format!("Query error: {}", e))?;
                    if columns.is_empty() {
                        for col in row.columns() {
                            columns.push(ColumnInfo {
                                name: col.name().to_string(),
                                type_name: col.type_info().name().to_string(),
                            });
                        }
                    }
                    result_rows.push(parse_mysql_row(&row));
                    row_count += 1;
                    if let Some(ref cb) = on_progress {
                        if row_count % 1000 == 0 {
                            cb(row_count);
                        }
                    }
                }
                Ok(RawQueryResult {
                    columns,
                    rows: result_rows,
                    rows_affected: row_count,
                    is_select: true,
                })
            }
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
        };

        if let Some(qid) = query_id {
            self.running_queries.write().remove(qid);
        }

        result
    }

    fn get_thread_id_for_query(&self, query_id: &str) -> Option<u64> {
        self.running_queries.read().get(query_id).copied()
    }

    async fn kill_query(&self, thread_id: u64) -> Result<(), String> {
        let kill_sql = format!("KILL QUERY {}", thread_id);
        sqlx::query(&kill_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
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
        // Relax strict date/group-by rules for the duration of the import so that
        // dumps containing '0000-00-00' defaults (e.g. WordPress) don't fail.
        // This mirrors what mysqldump prepends to its output.
        let _ = conn.execute("SET SESSION sql_mode='NO_AUTO_VALUE_ON_ZERO'").await;
        // Disable autocommit so all DML in this batch is committed in one shot,
        // avoiding a costly fsync per statement. DDL still triggers implicit commits.
        let _ = conn.execute("SET autocommit=0").await;

        let mut results = Vec::with_capacity(statements.len());
        for stmt in statements {
            results.push(
                conn.execute(stmt.as_str())
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
            );
        }

        let _ = conn.execute("COMMIT").await;
        let _ = conn.execute("SET autocommit=1").await;
        let _ = conn.execute("SET FOREIGN_KEY_CHECKS=1").await;
        let _ = conn.execute("SET SESSION sql_mode=@@GLOBAL.sql_mode").await;
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

    // Step 1: Auto-detect primary key when no sort is specified
    let effective_sort_column = if sort_column.is_none() {
        let pk_sql = format!(
            "SELECT column_name FROM information_schema.statistics WHERE table_schema='{}' AND table_name='{}' AND index_name='PRIMARY' ORDER BY seq_in_index ASC",
            database.replace('\'', "\\'"), table.replace('\'', "\\'")
        );
        let t0 = std::time::Instant::now();
        let pk_result = driver.get_primary_key_columns(&database, &table).await;
        let ms = t0.elapsed().as_millis() as u64;
        state.emit_query_log(&pk_sql, ms, pk_result.as_ref().err().map(|e| e.as_str()));
        pk_result.ok().and_then(|cols| cols.into_iter().next())
    } else {
        sort_column
    };

    // Step 2: Estimated row count from information_schema (fast, may be stale)
    let est_sql = format!(
        "SELECT table_rows as count FROM information_schema.TABLES WHERE TABLE_SCHEMA='{}' AND TABLE_NAME='{}'",
        database.replace('\'', "\\'"), table.replace('\'', "\\'")
    );
    let t0 = std::time::Instant::now();
    let _ = driver.get_estimated_row_count(&database, &table).await;
    let ms = t0.elapsed().as_millis() as u64;
    state.emit_query_log(&est_sql, ms, None);

    // Build WHERE clause string for query log (FilterSet is Clone)
    let (where_clause, _) = if let Some(ref f) = filters {
        crate::query_builder::build_where_clause(f)
    } else {
        (String::new(), vec![])
    };

    // order_sql starts with " ORDER BY" (space included), where_clause starts with " WHERE" or is empty
    let order_sql = match &effective_sort_column {
        Some(col) => format!(" ORDER BY `{}` {}", col, if sort_desc.unwrap_or(false) { "DESC" } else { "ASC" }),
        None => String::new(),
    };

    // Step 3: Exact COUNT(*)
    let count_sql = format!(
        "SELECT COUNT(*) as total FROM `{}`.`{}`{}",
        database, table, where_clause
    );

    // Step 4: SELECT *
    let select_sql = format!(
        "SELECT * FROM `{}`.`{}`{}{} LIMIT {} OFFSET {}",
        database, table, where_clause, order_sql, page_size, page * page_size
    );

    // Run the actual query (driver does COUNT + SELECT internally)
    let t0 = std::time::Instant::now();
    let result = driver
        .get_table_data(&database, &table, page, page_size, filters, effective_sort_column, sort_desc)
        .await;
    let ms = t0.elapsed().as_millis() as u64;

    state.emit_query_log(&count_sql, ms, result.as_ref().err().map(|e| e.as_str()));
    state.emit_query_log(&select_sql, ms, result.as_ref().err().map(|e| e.as_str()));

    result
}

/// Fire-and-forget: returns immediately and emits `query-result:{query_id}` when done.
/// This keeps the frontend UI responsive during long-running queries.
#[tauri::command]
pub fn execute_query(
    state: State<'_, AppState>,
    connection_id: Uuid,
    database: Option<String>,
    sql: String,
    query_id: String,
) -> Result<(), String> {
    let (env, allow_writes) = {
        let configs = state.connections_config.read();
        configs
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
        let on_progress: Option<Arc<dyn Fn(u64) + Send + Sync>> = Some(Arc::new(move |rows: u64| {
            use tauri::Emitter;
            let _ = progress_handle.emit(
                &format!("query-progress:{}", progress_qid),
                serde_json::json!({ "rows_fetched": rows }),
            );
        }));

        let chunk_handle = app_handle.clone();
        let chunk_qid = query_id.clone();
        let on_chunk: Option<Arc<dyn Fn(Option<Vec<crate::driver::ColumnInfo>>, Vec<serde_json::Value>) + Send + Sync>> =
            Some(Arc::new(move |columns: Option<Vec<crate::driver::ColumnInfo>>, rows: Vec<serde_json::Value>| {
                use tauri::Emitter;
                let _ = chunk_handle.emit(
                    &format!("query-chunk:{}", chunk_qid),
                    serde_json::json!({ "columns": columns, "rows": rows }),
                );
            }));

        let result = driver.execute_query(database.as_deref(), &sql, Some(&query_id), on_progress, on_chunk).await;
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
        let _ = app_handle.emit("query-log", serde_json::json!({
            "sql": sql,
            "timestamp": now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            "duration_ms": ms,
            "error": err_msg,
        }));
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
    if let Some(thread_id) = driver.get_thread_id_for_query(&query_id) {
        driver.kill_query(thread_id).await?;
    }
    Ok(())
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
    use std::io::{BufWriter, Write};
    use tokio::sync::mpsc;

    if format != "csv" && format != "json" && format != "sql" {
        return Err(format!("Unknown format: {}", format));
    }

    let _ = window.emit("export-progress", ExportProgress {
        current: 0,
        total: 100,
        status: format!("Streaming data from {}...", table),
    });

    let driver = state.get_driver(&connection_id)?;

    // Channel: producer streams rows, consumer writes to disk
    let (tx, mut rx) = mpsc::channel::<(Option<Vec<ColumnInfo>>, Value)>(512);

    let db_clone = database.clone();
    let table_clone = table.clone();
    let driver_clone = driver.clone();
    let stream_handle = tokio::spawn(async move {
        driver_clone.stream_all_rows(&db_clone, &table_clone, tx).await
    });

    let file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    let mut writer = BufWriter::new(file);
    let mut columns: Vec<ColumnInfo> = Vec::new();
    let mut row_count: usize = 0;
    let mut header_written = false;

    while let Some((col_opt, row)) = rx.recv().await {
        if let Some(cols) = col_opt {
            columns = cols;
        }

        row_count += 1;

        if row_count % 5000 == 0 {
            let _ = window.emit("export-progress", ExportProgress {
                current: 50,
                total: 100,
                status: format!("Writing row {}...", row_count),
            });
        }

        match format.as_str() {
            "csv" => {
                if !header_written {
                    let header: Vec<String> = columns.iter().map(|c| escape_csv(&c.name)).collect();
                    writeln!(writer, "{}", header.join(","))
                        .map_err(|e| format!("Write error: {}", e))?;
                    header_written = true;
                }
                if let Value::Object(ref map) = row {
                    let values: Vec<String> = columns.iter().map(|c| {
                        match map.get(&c.name) {
                            Some(Value::Null) | None => String::new(),
                            Some(Value::String(s)) => escape_csv(s),
                            Some(Value::Bool(b)) => b.to_string(),
                            Some(v) => v.to_string(),
                        }
                    }).collect();
                    writeln!(writer, "{}", values.join(","))
                        .map_err(|e| format!("Write error: {}", e))?;
                }
            }
            "json" => {
                if !header_written {
                    writer.write_all(b"[\n")
                        .map_err(|e| format!("Write error: {}", e))?;
                    header_written = true;
                } else {
                    writer.write_all(b",\n")
                        .map_err(|e| format!("Write error: {}", e))?;
                }
                let row_str = serde_json::to_string(&row)
                    .map_err(|e| format!("Serialization error: {}", e))?;
                writer.write_all(row_str.as_bytes())
                    .map_err(|e| format!("Write error: {}", e))?;
            }
            "sql" => {
                if !header_written {
                    writeln!(writer, "-- Export of `{}`.`{}`\n", database, table)
                        .map_err(|e| format!("Write error: {}", e))?;
                    header_written = true;
                }
                if let Value::Object(ref map) = row {
                    let col_names = columns.iter()
                        .map(|c| format!("`{}`", c.name))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let values: Vec<String> = columns.iter().map(|c| {
                        match map.get(&c.name) {
                            Some(Value::Null) | None => "NULL".to_string(),
                            Some(Value::String(s)) => format!("'{}'", s.replace('\'', "\\'")),
                            Some(Value::Bool(b)) => if *b { "1".to_string() } else { "0".to_string() },
                            Some(v) => v.to_string(),
                        }
                    }).collect();
                    writeln!(writer, "INSERT INTO `{}` ({}) VALUES ({});", table, col_names, values.join(", "))
                        .map_err(|e| format!("Write error: {}", e))?;
                }
            }
            _ => unreachable!(),
        }
    }

    // Close JSON array
    if format == "json" {
        if header_written {
            writer.write_all(b"\n]").map_err(|e| format!("Write error: {}", e))?;
        } else {
            writer.write_all(b"[]").map_err(|e| format!("Write error: {}", e))?;
        }
    }

    writer.flush().map_err(|e| format!("Write error: {}", e))?;

    // Propagate any streaming error
    stream_handle.await
        .map_err(|e| format!("Stream task error: {}", e))??;

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
