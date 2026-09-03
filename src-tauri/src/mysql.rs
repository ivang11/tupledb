use crate::driver::{
    ColumnInfo, ColumnStructure, DatabaseCollation, DatabaseCreationOptions, DatabaseDriver,
    ForeignKey, KeysetPage, QueryChunkCallback, QueryResult, RawQueryResult, RowChange,
    RowDeletion, Table, TableChange, TableDataTimings, TableIndex,
};
use crate::filters::FilterSet;
use crate::state::AppState;
use async_trait::async_trait;
use chrono::Timelike;
use futures::StreamExt;
use parking_lot::RwLock;
use serde_json::{Map, Value};
use sqlx::{Column, MySql, MySqlPool, Row, TypeInfo, ValueRef};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

// --------------------------------------------------------------------------
// WKB → WKT parser (MySQL prepends a 4-byte SRID to standard WKB)
// --------------------------------------------------------------------------

fn read_u32_wkb(data: &[u8], le: bool) -> u32 {
    let b = [data[0], data[1], data[2], data[3]];
    if le {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    }
}

fn read_f64_wkb(data: &[u8], le: bool) -> f64 {
    let b = [
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ];
    if le {
        f64::from_le_bytes(b)
    } else {
        f64::from_be_bytes(b)
    }
}

fn wkb_parse(data: &[u8]) -> Option<String> {
    if data.len() < 5 {
        return None;
    }
    let le = data[0] == 1;
    let geom_type = read_u32_wkb(&data[1..5], le);
    let payload = &data[5..];
    match geom_type {
        1 => {
            // Point
            if payload.len() < 16 {
                return None;
            }
            let x = read_f64_wkb(&payload[0..8], le);
            let y = read_f64_wkb(&payload[8..16], le);
            Some(format!("POINT({} {})", x, y))
        }
        2 => {
            // LineString
            if payload.len() < 4 {
                return None;
            }
            let n = read_u32_wkb(&payload[0..4], le) as usize;
            let coords = &payload[4..];
            if coords.len() < n * 16 {
                return None;
            }
            let pts: Vec<String> = (0..n)
                .map(|i| {
                    let x = read_f64_wkb(&coords[i * 16..i * 16 + 8], le);
                    let y = read_f64_wkb(&coords[i * 16 + 8..i * 16 + 16], le);
                    format!("{} {}", x, y)
                })
                .collect();
            Some(format!("LINESTRING({})", pts.join(", ")))
        }
        3 => {
            // Polygon
            if payload.len() < 4 {
                return None;
            }
            let n_rings = read_u32_wkb(&payload[0..4], le) as usize;
            let mut offset = 4usize;
            let mut rings = Vec::new();
            for _ in 0..n_rings {
                if payload.len() < offset + 4 {
                    return None;
                }
                let n_pts = read_u32_wkb(&payload[offset..offset + 4], le) as usize;
                offset += 4;
                if payload.len() < offset + n_pts * 16 {
                    return None;
                }
                let pts: Vec<String> = (0..n_pts)
                    .map(|i| {
                        let x = read_f64_wkb(&payload[offset + i * 16..offset + i * 16 + 8], le);
                        let y =
                            read_f64_wkb(&payload[offset + i * 16 + 8..offset + i * 16 + 16], le);
                        format!("{} {}", x, y)
                    })
                    .collect();
                offset += n_pts * 16;
                rings.push(format!("({})", pts.join(", ")));
            }
            Some(format!("POLYGON({})", rings.join(", ")))
        }
        _ => None,
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

fn sql_literal(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(v) => {
            if *v {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Value::Number(v) => v.to_string(),
        Value::String(v) => format!("'{}'", v.replace('\\', "\\\\").replace('\'', "\\'")),
        other => format!(
            "'{}'",
            other.to_string().replace('\\', "\\\\").replace('\'', "\\'")
        ),
    }
}

fn quote_identifier(identifier: &str) -> Result<String, String> {
    let trimmed = identifier.trim();
    if trimmed.is_empty() {
        return Err("Identifier cannot be empty".to_string());
    }
    if trimmed.chars().count() > 64 || trimmed.contains('\0') {
        return Err("Identifier must contain at most 64 characters".to_string());
    }
    Ok(format!("`{}`", trimmed.replace('`', "``")))
}

fn validate_column_type(column_type: &str) -> Result<String, String> {
    const TYPES: &[&str] = &[
        "BIGINT", "BINARY", "BIT", "BLOB", "BOOL", "BOOLEAN", "CHAR", "DATE",
        "DATETIME", "DEC", "DECIMAL", "DOUBLE", "ENUM", "FIXED", "FLOAT", "GEOMETRY",
        "GEOMETRYCOLLECTION", "INT", "INTEGER", "JSON", "LINESTRING", "LONGBLOB",
        "LONGTEXT", "MEDIUMBLOB", "MEDIUMINT", "MEDIUMTEXT", "MULTILINESTRING",
        "MULTIPOINT", "MULTIPOLYGON", "NUMERIC", "POINT", "POLYGON", "REAL", "SET",
        "SMALLINT", "TEXT", "TIME", "TIMESTAMP", "TINYBLOB", "TINYINT", "TINYTEXT",
        "VARBINARY", "VARCHAR", "YEAR",
    ];

    let value = column_type.trim();
    if value.is_empty() || value.len() > 1_000 {
        return Err("Enter a valid column type".to_string());
    }
    if value.contains(';')
        || value.contains('`')
        || value.contains('\0')
        || value.contains("--")
        || value.contains("/*")
        || value.contains("*/")
        || value.contains('#')
    {
        return Err("Column type contains unsupported SQL syntax".to_string());
    }

    let base_end = value
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(value.len());
    let base = value[..base_end].to_ascii_uppercase();
    if !TYPES.contains(&base.as_str()) {
        return Err(format!("Unsupported column type: {}", &value[..base_end]));
    }

    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' {
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Err("Column type has unbalanced parentheses".to_string()),
            ')' => depth -= 1,
            ',' if depth == 0 => {
                return Err("Column type contains unsupported SQL syntax".to_string())
            }
            c if c.is_ascii_alphanumeric()
                || c.is_ascii_whitespace()
                || matches!(c, '_' | ',' | '.' | '+' | '-') => {}
            _ => return Err("Column type contains unsupported characters".to_string()),
        }
    }
    if quoted || depth != 0 {
        return Err("Column type has an unfinished quote or parenthesis".to_string());
    }

    let suffix_start = if let Some(open) = value.find('(') {
        let mut suffix_depth = 0usize;
        let mut end = None;
        let mut in_quote = false;
        let mut suffix_escaped = false;
        for (offset, ch) in value[open..].char_indices() {
            if suffix_escaped {
                suffix_escaped = false;
                continue;
            }
            if in_quote && ch == '\\' {
                suffix_escaped = true;
                continue;
            }
            if ch == '\'' {
                in_quote = !in_quote;
            } else if !in_quote && ch == '(' {
                suffix_depth += 1;
            } else if !in_quote && ch == ')' {
                suffix_depth -= 1;
                if suffix_depth == 0 {
                    end = Some(open + offset + ch.len_utf8());
                    break;
                }
            }
        }
        end.unwrap_or(value.len())
    } else {
        base_end
    };
    let modifiers = value[suffix_start..].trim();
    if !modifiers.is_empty()
        && !modifiers
            .split_whitespace()
            .all(|word| matches!(word.to_ascii_uppercase().as_str(), "UNSIGNED" | "ZEROFILL"))
    {
        return Err("Only UNSIGNED and ZEROFILL modifiers are supported".to_string());
    }

    Ok(value.to_string())
}

fn quote_mysql_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn column_type_supports_charset(column_type: &str) -> bool {
    let base_end = column_type
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(column_type.len());
    matches!(
        column_type[..base_end].to_ascii_uppercase().as_str(),
        "CHAR"
            | "VARCHAR"
            | "TINYTEXT"
            | "TEXT"
            | "MEDIUMTEXT"
            | "LONGTEXT"
            | "ENUM"
            | "SET"
    )
}

fn append_keyset_predicate(
    where_clause: &str,
    keyset: &KeysetPage,
    descending_query: bool,
) -> String {
    let op = if descending_query { "<" } else { ">" };
    let predicate = format!("`{}` {} {}", keyset.column, op, sql_literal(&keyset.value));
    if where_clause.trim().is_empty() {
        format!(" WHERE {}", predicate)
    } else {
        format!("{} AND {}", where_clause, predicate)
    }
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
    // Decode by ordinal, not by column name. MySQL can return fresh column
    // metadata while a pooled/prepared statement still has a stale name index
    // after a schema change. Mixing `row.columns()` with name-based lookups can
    // then read a value from a different column until another connection is
    // used. The ordinal and its metadata always come from the same result row.
    for (index, col) in row.columns().iter().enumerate() {
        let col_name = col.name();
        let type_name = col.type_info().name().to_uppercase();
        let value: Value = match row.try_get_raw(index) {
            Ok(raw) if raw.is_null() => Value::Null,
            _ => match type_name.as_str() {
                "TINYINT(1)" | "BOOLEAN" | "BOOL" => row
                    .try_get::<i8, _>(index)
                    .map(|v| Value::Bool(v != 0))
                    .unwrap_or(Value::Null),
                t if t == "BIT" || t.starts_with("BIT(") => {
                    let width: u32 = t
                        .trim_start_matches("BIT(")
                        .trim_end_matches(')')
                        .parse()
                        .unwrap_or(1);
                    let to_bin =
                        |n: u64| Value::String(format!("{:0>width$b}", n, width = width as usize));
                    row.try_get::<u64, _>(index)
                        .map(&to_bin)
                        .or_else(|_| {
                            row.try_get::<Vec<u8>, _>(index).map(|b| {
                                let n = b.iter().fold(0u64, |acc, &x| (acc << 8) | x as u64);
                                to_bin(n)
                            })
                        })
                        .unwrap_or(Value::Null)
                }
                t if t.contains("INT") => row
                    .try_get::<i64, _>(index)
                    .map(|v| Value::Number(v.into()))
                    .or_else(|_| {
                        row.try_get::<u64, _>(index)
                            .map(|v| Value::Number(v.into()))
                    })
                    .unwrap_or(Value::Null),
                t if t == "DOUBLE"
                    || t == "FLOAT"
                    || t.starts_with("DOUBLE")
                    || t.starts_with("FLOAT") =>
                {
                    row.try_get::<f64, _>(index)
                        .ok()
                        .and_then(serde_json::Number::from_f64)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
                t if t.starts_with("DECIMAL") || t.starts_with("NUMERIC") || t == "NEWDECIMAL" => {
                    row.try_get::<rust_decimal::Decimal, _>(index)
                        .map(|d| Value::String(d.to_string()))
                        .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                        .unwrap_or(Value::Null)
                }
                "YEAR" => row
                    .try_get::<u16, _>(index)
                    .map(|v| Value::Number(v.into()))
                    .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                    .unwrap_or(Value::Null),
                "DATE" => row
                    .try_get::<chrono::NaiveDate, _>(index)
                    .map(|d| Value::String(d.to_string()))
                    .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                    .unwrap_or(Value::Null),
                t if t == "TIME" || t.starts_with("TIME(") => row
                    .try_get::<chrono::NaiveTime, _>(index)
                    .map(|t| {
                        let base = t.format("%H:%M:%S").to_string();
                        if t.nanosecond() == 0 {
                            base
                        } else {
                            let frac = format!("{:.6}", t.nanosecond() as f64 / 1_000_000_000.0);
                            format!(
                                "{}{}",
                                base,
                                frac.trim_start_matches('0').trim_end_matches('0')
                            )
                        }
                    })
                    .map(Value::String)
                    .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                    .unwrap_or(Value::Null),
                t if t == "DATETIME" || t.starts_with("DATETIME(") => row
                    .try_get::<chrono::NaiveDateTime, _>(index)
                    .map(|dt| {
                        let base = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                        if dt.nanosecond() == 0 {
                            base
                        } else {
                            let frac = format!("{:.6}", dt.nanosecond() as f64 / 1_000_000_000.0);
                            format!(
                                "{}{}",
                                base,
                                frac.trim_start_matches('0').trim_end_matches('0')
                            )
                        }
                    })
                    .map(Value::String)
                    .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                    .unwrap_or(Value::Null),
                t if t == "TIMESTAMP" || t.starts_with("TIMESTAMP(") => {
                    let s = row
                        .try_get::<chrono::NaiveDateTime, _>(index)
                        .map(|dt| {
                            let base = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                            if dt.nanosecond() == 0 {
                                base
                            } else {
                                let frac =
                                    format!("{:.6}", dt.nanosecond() as f64 / 1_000_000_000.0);
                                format!(
                                    "{}{}",
                                    base,
                                    frac.trim_start_matches('0').trim_end_matches('0')
                                )
                            }
                        })
                        .or_else(|_| {
                            row.try_get::<chrono::DateTime<chrono::Utc>, _>(index)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        })
                        .or_else(|_| row.try_get::<String, _>(index));
                    s.map(Value::String).unwrap_or(Value::Null)
                }
                t if t.contains("BLOB") || t == "BINARY" || t.starts_with("VARBINARY") => row
                    .try_get::<Vec<u8>, _>(index)
                    .map(|b| {
                        let trimmed: Vec<u8> = b
                            .iter()
                            .copied()
                            .rev()
                            .skip_while(|&x| x == 0)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        match String::from_utf8(trimmed) {
                            Ok(s) => Value::String(s),
                            Err(_) => {
                                let hex: String =
                                    b.iter().map(|byte| format!("{:02x}", byte)).collect();
                                Value::String(format!("0x{}", hex))
                            }
                        }
                    })
                    .unwrap_or(Value::Null),
                t if t == "GEOMETRY"
                    || t == "POINT"
                    || t == "LINESTRING"
                    || t == "POLYGON"
                    || t.starts_with("MULTI")
                    || t == "GEOMETRYCOLLECTION" =>
                {
                    row.try_get_unchecked::<Vec<u8>, _>(index)
                        .map(|b| Value::String(mysql_wkb_to_wkt(&b)))
                        .or_else(|_| row.try_get_unchecked::<String, _>(index).map(Value::String))
                        .unwrap_or_else(|_| Value::String(format!("<{}>", type_name)))
                }
                t if t == "JSON" || t.contains("JSON") => row
                    .try_get_unchecked::<String, _>(index)
                    .or_else(|_| {
                        row.try_get_unchecked::<Vec<u8>, _>(index)
                            .map(|b| String::from_utf8_lossy(&b).to_string())
                    })
                    .map(Value::String)
                    .unwrap_or_else(|_| Value::String(format!("<{}>", type_name))),
                _ => row
                    .try_get::<String, _>(index)
                    .map(Value::String)
                    .or_else(|_| {
                        row.try_get::<Vec<u8>, _>(index)
                            .map(|b| Value::String(String::from_utf8_lossy(&b).to_string()))
                    })
                    .unwrap_or_else(|_| Value::String(format!("<{}>", type_name))),
            },
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
    get_optional_str_lossy(row, index).unwrap_or_else(|| "unknown".to_string())
}

fn get_optional_str_lossy(row: &sqlx::mysql::MySqlRow, index: usize) -> Option<String> {
    if row.try_get_raw(index).ok().is_some_and(|value| value.is_null()) {
        return None;
    }
    row.try_get::<String, _>(index).ok().or_else(|| {
        row.try_get::<Vec<u8>, _>(index)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    })
}

fn is_early_connection_close(error: &sqlx::Error) -> bool {
    let msg = error.to_string().to_lowercase();
    msg.contains("got 0 bytes at eof")
        || msg.contains("early eof")
        || msg.contains("connection reset")
        || msg.contains("connection closed")
}

// --------------------------------------------------------------------------
// MySqlDriver
// --------------------------------------------------------------------------

pub struct MySqlDriver {
    pool: MySqlPool,
    running_queries: Arc<RwLock<HashMap<String, u64>>>,
    running_imports: Arc<RwLock<HashMap<String, u64>>>,
    import_sessions: Arc<RwLock<HashMap<String, Arc<ImportSession>>>>,
    /// True when the server has ONLY_FULL_GROUP_BY enabled (MySQL 5.7+).
    /// Used to disable it for the session in read-only export queries so that
    /// VIEWs created without strict mode can still be read.
    no_group_by_check: bool,
}

struct ImportSession {
    conn: tokio::sync::Mutex<sqlx::pool::PoolConnection<MySql>>,
    max_batch_bytes: usize,
}

impl MySqlDriver {
    pub fn new(pool: MySqlPool, no_group_by_check: bool) -> Self {
        Self {
            pool,
            running_queries: Arc::new(RwLock::new(HashMap::new())),
            running_imports: Arc::new(RwLock::new(HashMap::new())),
            import_sessions: Arc::new(RwLock::new(HashMap::new())),
            no_group_by_check,
        }
    }
}

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    // --- Schema ---

    async fn get_databases(&self) -> Result<Vec<String>, String> {
        let query = "SELECT schema_name FROM information_schema.schemata ORDER BY schema_name ASC";
        let rows = match sqlx::query(query).fetch_all(&self.pool).await {
            Ok(rows) => rows,
            Err(first_error) if is_early_connection_close(&first_error) => {
                println!(
                    "  -> MySQL closed connection while fetching databases, retrying once: {}",
                    first_error
                );
                sqlx::query(query)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| format!("Failed to fetch databases: {}", e))?
            }
            Err(e) => return Err(format!("Failed to fetch databases: {}", e)),
        };
        Ok(rows.iter().map(|row| get_str_lossy(row, 0)).collect())
    }

    async fn get_database_creation_options(&self) -> Result<DatabaseCreationOptions, String> {
        let defaults = sqlx::query("SELECT @@character_set_server, @@collation_server")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch database defaults: {}", e))?;
        let collations = match sqlx::query(
            "SELECT COLLATION_NAME, CHARACTER_SET_NAME, IS_DEFAULT FROM information_schema.COLLATIONS ORDER BY CHARACTER_SET_NAME, COLLATION_NAME",
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => rows
                .iter()
                .map(|row| DatabaseCollation {
                    name: get_str_lossy(row, 0),
                    character_set: get_str_lossy(row, 1),
                    is_default: get_str_lossy(row, 2).eq_ignore_ascii_case("yes"),
                })
                .collect(),
            Err(information_schema_error) => {
                // Some managed MySQL-compatible services restrict information_schema
                // while still exposing the equivalent SHOW command.
                let rows = sqlx::query("SHOW COLLATION")
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|show_error| {
                        format!(
                            "Failed to fetch available collations (information_schema: {}; SHOW COLLATION: {})",
                            information_schema_error, show_error
                        )
                    })?;
                rows.iter()
                    .map(|row| DatabaseCollation {
                        name: get_str_lossy(row, 0),
                        character_set: get_str_lossy(row, 1),
                        is_default: get_str_lossy(row, 3).eq_ignore_ascii_case("yes"),
                    })
                    .collect()
            }
        };

        Ok(DatabaseCreationOptions {
            default_character_set: get_str_lossy(&defaults, 0),
            default_collation: get_str_lossy(&defaults, 1),
            collations,
        })
    }

    async fn create_database(
        &self,
        name: &str,
        character_set: Option<&str>,
        collation: Option<&str>,
    ) -> Result<(), String> {
        let mut query = format!("CREATE DATABASE `{}`", name);

        if character_set.is_some() || collation.is_some() {
            let options = self.get_database_creation_options().await?;
            let selected_collation = collation.and_then(|value| {
                options
                    .collations
                    .iter()
                    .find(|option| option.name == value)
            });

            if let Some(value) = character_set {
                let is_valid = options
                    .collations
                    .iter()
                    .any(|option| option.character_set == value);
                if !is_valid {
                    return Err(format!("Unsupported character set: {}", value));
                }
                query.push_str(&format!(" CHARACTER SET {}", value));
            }

            if let Some(value) = collation {
                let option = selected_collation
                    .ok_or_else(|| format!("Unsupported collation: {}", value))?;
                if let Some(selected_character_set) = character_set {
                    if option.character_set != selected_character_set {
                        return Err(format!(
                            "Collation {} does not belong to character set {}",
                            value, selected_character_set
                        ));
                    }
                }
                query.push_str(&format!(" COLLATE {}", value));
            }
        }

        sqlx::query(&query)
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
        let mut tables: Vec<Table> = rows
            .iter()
            .map(|row| Table {
                name: get_str_lossy(row, 0),
                table_type: get_str_lossy(row, 1),
            })
            .collect();
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
        Ok(rows
            .iter()
            .map(|row| ColumnStructure {
                field: get_str_lossy(row, 0),
                field_type: get_str_lossy(row, 1),
                nullable: get_str_lossy(row, 2) == "YES",
                key: get_str_lossy(row, 3),
                default_value: get_optional_str_lossy(row, 4),
                extra: get_str_lossy(row, 5),
            })
            .collect())
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
        Ok(rows
            .iter()
            .map(|row| ForeignKey {
                column: get_str_lossy(row, 0),
                referenced_table: get_str_lossy(row, 1),
                referenced_column: get_str_lossy(row, 2),
            })
            .collect())
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
        Ok(rows
            .iter()
            .map(|row| {
                let non_unique: bool = row.try_get::<i64, _>(1)
                    .or_else(|_| row.try_get::<u64, _>(1).map(|v| v as i64))
                    .map(|v| v != 0)
                    .unwrap_or_else(|_| get_str_lossy(row, 1) != "0");
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
            })
            .collect())
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

    async fn get_estimated_row_count(&self, database: &str, table: &str) -> Result<i64, String> {
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
        Ok(row
            .map(|r| get_str_lossy(&r, 0).parse().unwrap_or(0))
            .unwrap_or(0))
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
        exact_count: bool,
        keyset: Option<KeysetPage>,
    ) -> Result<QueryResult, String> {
        let (where_clause, params) = if let Some(f) = filters {
            crate::query_builder::build_where_clause(&f)
        } else {
            ("".to_string(), vec![])
        };

        let offset = page * page_size;
        let keyset_desc_query = keyset
            .as_ref()
            .map(|k| k.direction == "prev")
            .unwrap_or(false);
        let data_where_clause = keyset
            .as_ref()
            .map(|k| append_keyset_predicate(&where_clause, k, keyset_desc_query))
            .unwrap_or_else(|| where_clause.clone());
        let total_timer = std::time::Instant::now();
        let count_timer = std::time::Instant::now();
        let should_use_exact_count = exact_count || !where_clause.is_empty();
        let total_count_is_estimate = !should_use_exact_count;

        let mut total_count = if should_use_exact_count {
            let count_query = format!(
                "SELECT COUNT(*) as total FROM `{}`.`{}` {}",
                database, table, where_clause
            );
            let mut q = sqlx::query_as::<_, (i64,)>(&count_query);
            for p in &params {
                q = q.bind(p);
            }
            let (total_count,) = q
                .fetch_one(&self.pool)
                .await
                .map_err(|e| format!("Failed to fetch total count: {}", e))?;
            total_count
        } else {
            self.get_estimated_row_count(database, table).await?
        };
        let count_ms = count_timer.elapsed().as_millis() as u64;

        let order_sql = match (&keyset, &sort_column) {
            (Some(k), _) => {
                let dir = if keyset_desc_query { "DESC" } else { "ASC" };
                format!(" ORDER BY `{}` {}", k.column, dir)
            }
            (None, Some(col)) => {
                let desc = sort_desc.unwrap_or(false);
                format!(" ORDER BY `{}` {}", col, if desc { "DESC" } else { "ASC" })
            }
            (None, None) => String::new(),
        };

        let data_query = if keyset.is_some() {
            format!(
                "SELECT * FROM `{}`.`{}` {}{} LIMIT {}",
                database, table, data_where_clause, order_sql, page_size
            )
        } else {
            format!(
                "SELECT * FROM `{}`.`{}` {}{} LIMIT {} OFFSET {}",
                database, table, data_where_clause, order_sql, page_size, offset
            )
        };
        let mut q = sqlx::query(&data_query);
        for p in &params {
            q = q.bind(p);
        }
        let select_timer = std::time::Instant::now();
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch data: {}", e))?;
        let select_ms = select_timer.elapsed().as_millis() as u64;

        let (columns, mut result_rows) = rows_to_parsed(rows);
        if keyset_desc_query {
            result_rows.reverse();
        }
        if total_count_is_estimate {
            let visible_minimum = offset as i64 + result_rows.len() as i64;
            total_count = total_count.max(visible_minimum);
        }
        Ok(QueryResult {
            columns,
            rows: result_rows,
            total_count,
            total_count_is_estimate,
            timings: Some(TableDataTimings {
                count_ms,
                select_ms,
                total_ms: total_timer.elapsed().as_millis() as u64,
            }),
        })
    }

    async fn get_all_rows(
        &self,
        database: &str,
        table: &str,
    ) -> Result<(Vec<ColumnInfo>, Vec<Value>), String> {
        let query = format!("SELECT * FROM `{}`.`{}`", database, table);
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if self.no_group_by_check {
            sqlx::query(
                "SET SESSION sql_mode=(SELECT REPLACE(@@SESSION.sql_mode,'ONLY_FULL_GROUP_BY',''))",
            )
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
            sqlx::query(
                "SET SESSION sql_mode=(SELECT REPLACE(@@SESSION.sql_mode,'ONLY_FULL_GROUP_BY',''))",
            )
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
        on_chunk: Option<QueryChunkCallback>,
        max_retained_cells: Option<usize>,
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
            self.running_queries
                .write()
                .insert(qid.to_string(), thread_id);
        }

        let result = async {
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
                        if (row_count as usize)
                            < retained_row_limit(columns.len(), max_retained_cells)
                        {
                            chunk_buf.push(parse_mysql_row(&row));
                        }
                        row_count += 1;

                        if chunk_buf.len() >= CHUNK_SIZE {
                            if let Some(ref cb) = on_chunk {
                                let cols = if first_chunk {
                                    Some(columns.clone())
                                } else {
                                    None
                                };
                                cb(
                                    cols,
                                    std::mem::replace(
                                        &mut chunk_buf,
                                        Vec::with_capacity(CHUNK_SIZE),
                                    ),
                                );
                                first_chunk = false;
                            }
                            if let Some(ref cb) = on_progress {
                                cb(row_count);
                            }
                        } else if row_count.is_multiple_of(1000) {
                            if let Some(ref cb) = on_progress {
                                cb(row_count);
                            }
                        }
                    }
                    // Flush remaining rows
                    if !chunk_buf.is_empty() {
                        if let Some(ref cb) = on_chunk {
                            let cols = if first_chunk {
                                Some(columns.clone())
                            } else {
                                None
                            };
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
                            if row_count.is_multiple_of(1000) {
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
            }
        }
        .await;

        if let Some(qid) = query_id {
            self.running_queries.write().remove(qid);
        }

        result
    }

    fn get_thread_id_for_query(&self, query_id: &str) -> Option<u64> {
        self.running_queries.read().get(query_id).copied()
    }

    fn get_thread_id_for_import(&self, import_id: &str) -> Option<u64> {
        self.running_imports.read().get(import_id).copied()
    }

    async fn kill_query(&self, thread_id: u64) -> Result<(), String> {
        let kill_sql = format!("KILL QUERY {}", thread_id);
        sqlx::query(&kill_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn kill_connection(&self, thread_id: u64) -> Result<(), String> {
        let kill_sql = format!("KILL CONNECTION {}", thread_id);
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
                        if let Some(i) = n.as_i64() {
                            q.bind(i)
                        } else if let Some(f) = n.as_f64() {
                            q.bind(f)
                        } else {
                            return Err("Invalid number".to_string());
                        }
                    }
                    Value::String(s) => q.bind(s),
                    _ => return Err("Unsupported value type".to_string()),
                };
                q = match update.pk_value.clone() {
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            q.bind(i)
                        } else {
                            return Err("Invalid PK number".to_string());
                        }
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
                    if let Some(i) = n.as_i64() {
                        q.bind(i)
                    } else {
                        return Err("Invalid PK number".to_string());
                    }
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
            matches!(
                upper.as_str(),
                "NOW()"
                    | "CURRENT_TIMESTAMP"
                    | "CURRENT_TIMESTAMP()"
                    | "CURRENT_DATE"
                    | "CURRENT_DATE()"
                    | "CURRENT_TIME"
                    | "CURRENT_TIME()"
                    | "UUID()"
                    | "NULL"
            )
        }

        let columns: Vec<String> = values.iter().map(|v| format!("`{}`", v.column)).collect();
        let placeholders: Vec<String> = values
            .iter()
            .map(|v| {
                if let Value::String(s) = &v.value {
                    if is_sql_expression(s) {
                        return s.trim().to_uppercase();
                    }
                }
                "?".to_string()
            })
            .collect();

        let query = format!(
            "INSERT INTO `{}`.`{}` ({}) VALUES ({})",
            database,
            table,
            columns.join(", "),
            placeholders.join(", ")
        );

        let mut q = sqlx::query(&query);
        for v in &values {
            if let Value::String(s) = &v.value {
                if is_sql_expression(s) {
                    continue;
                }
            }
            q = match &v.value {
                Value::Null => q.bind(None::<String>),
                Value::Bool(b) => q.bind(if *b { 1i64 } else { 0i64 }),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        q.bind(i)
                    } else if let Some(f) = n.as_f64() {
                        q.bind(f.to_string())
                    } else {
                        return Err("Invalid number".to_string());
                    }
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

    async fn alter_table_column(
        &self,
        database: &str,
        table: &str,
        old_name: &str,
        new_name: &str,
        new_type: &str,
    ) -> Result<String, String> {
        let database_sql = quote_identifier(database)?;
        let table_sql = quote_identifier(table)?;
        let old_name_sql = quote_identifier(old_name)?;
        let new_name_sql = quote_identifier(new_name)?;
        let new_type = validate_column_type(new_type)?;

        let metadata = sqlx::query(
            "SELECT COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT, EXTRA, COLUMN_COMMENT, \
                    CHARACTER_SET_NAME, COLLATION_NAME, GENERATION_EXPRESSION \
             FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND COLUMN_NAME = ?",
        )
        .bind(database)
        .bind(table)
        .bind(old_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to read column definition: {}", e))?
        .ok_or_else(|| format!("Column '{}' no longer exists", old_name))?;

        let current_type = get_optional_str_lossy(&metadata, 0)
            .ok_or_else(|| "Failed to read column type from information_schema".to_string())?;
        let name_changed = old_name.trim() != new_name.trim();
        let type_changed = current_type.trim().to_ascii_lowercase()
            != new_type.trim().to_ascii_lowercase();

        if !name_changed && !type_changed {
            return Ok(String::new());
        }

        // RENAME COLUMN is the safest path when the type is unchanged: MySQL
        // retains every attribute without us having to rebuild the definition.
        let sql = if !type_changed {
            format!(
                "ALTER TABLE {}.{} RENAME COLUMN {} TO {}",
                database_sql, table_sql, old_name_sql, new_name_sql
            )
        } else {
            let nullable = get_optional_str_lossy(&metadata, 1).ok_or_else(|| {
                "Failed to read column nullability from information_schema".to_string()
            })?;
            let default_value = get_optional_str_lossy(&metadata, 2);
            let extra = get_optional_str_lossy(&metadata, 3).unwrap_or_default();
            let comment = get_optional_str_lossy(&metadata, 4).unwrap_or_default();
            let character_set = get_optional_str_lossy(&metadata, 5);
            let collation = get_optional_str_lossy(&metadata, 6);
            let generation = get_optional_str_lossy(&metadata, 7).unwrap_or_default();
            let extra_upper = extra.to_ascii_uppercase();

            let mut definition = new_type;
            if column_type_supports_charset(&definition) {
                if let Some(charset) = character_set {
                    definition.push_str(" CHARACTER SET ");
                    definition.push_str(&quote_identifier(&charset)?);
                }
                if let Some(collation) = collation {
                    definition.push_str(" COLLATE ");
                    definition.push_str(&quote_identifier(&collation)?);
                }
            }

            if !generation.trim().is_empty() {
                definition.push_str(" GENERATED ALWAYS AS (");
                definition.push_str(&generation);
                definition.push(')');
                if extra_upper.contains("STORED GENERATED") {
                    definition.push_str(" STORED");
                } else {
                    definition.push_str(" VIRTUAL");
                }
            } else {
                definition.push_str(if nullable == "YES" { " NULL" } else { " NOT NULL" });

                if let Some(default) = default_value {
                    let upper = default.trim().to_ascii_uppercase();
                    let temporal_expression = upper == "CURRENT_TIMESTAMP"
                        || upper.starts_with("CURRENT_TIMESTAMP(")
                        || upper == "CURRENT_DATE"
                        || upper.starts_with("CURRENT_DATE(")
                        || upper == "CURRENT_TIME"
                        || upper.starts_with("CURRENT_TIME(");
                    definition.push_str(" DEFAULT ");
                    if temporal_expression {
                        definition.push_str(default.trim());
                    } else if extra_upper.contains("DEFAULT_GENERATED") {
                        if default.trim().starts_with('(') {
                            definition.push_str(default.trim());
                        } else {
                            definition.push('(');
                            definition.push_str(default.trim());
                            definition.push(')');
                        }
                    } else {
                        definition.push_str(&quote_mysql_string(&default));
                    }
                } else if nullable == "YES" && !extra_upper.contains("AUTO_INCREMENT") {
                    definition.push_str(" DEFAULT NULL");
                }

                if extra_upper.contains("AUTO_INCREMENT") {
                    definition.push_str(" AUTO_INCREMENT");
                }
                if let Some(on_update) = extra_upper.find("ON UPDATE ") {
                    definition.push(' ');
                    definition.push_str(&extra[on_update..]);
                }
                if extra_upper.contains("INVISIBLE") {
                    definition.push_str(" INVISIBLE");
                }
            }

            if !comment.is_empty() {
                definition.push_str(" COMMENT ");
                definition.push_str(&quote_mysql_string(&comment));
            }

            format!(
                "ALTER TABLE {}.{} CHANGE COLUMN {} {} {}",
                database_sql, table_sql, old_name_sql, new_name_sql, definition
            )
        };

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to alter column: {}", e))?;
        Ok(sql)
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

    async fn drop_tables(
        &self,
        database: &str,
        tables: &[String],
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        if tables.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        }

        for table in tables {
            sqlx::query(&format!("DROP TABLE `{}`.`{}`", database, table))
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        }

        if disable_fk_checks {
            let _ = sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
                .execute(&mut *conn)
                .await;
        }

        Ok(())
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

    async fn begin_import_session(&self, database: &str, import_id: &str) -> Result<(), String> {
        use sqlx::Executor;

        if self.import_sessions.read().contains_key(import_id) {
            return Ok(());
        }

        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;

        let use_query = format!("USE `{}`", database);
        conn.execute(use_query.as_str())
            .await
            .map_err(|e| format!("Failed to select database: {}", e))?;

        let thread_id: u64 = sqlx::query_scalar::<_, u64>("SELECT CONNECTION_ID()")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        self.running_imports
            .write()
            .insert(import_id.to_string(), thread_id);

        let max_allowed_packet: u64 = sqlx::query_scalar("SELECT @@max_allowed_packet")
            .fetch_one(&mut *conn)
            .await
            .unwrap_or(4 * 1024 * 1024);

        // Keep a safety margin so the final SQL block stays comfortably below
        // the server packet limit even with separators and protocol overhead.
        let max_batch_bytes =
            ((max_allowed_packet as usize) * 3 / 4).clamp(1024 * 1024, 32 * 1024 * 1024);

        conn.execute("SET FOREIGN_KEY_CHECKS=0")
            .await
            .map_err(|e| e.to_string())?;
        conn.execute("SET SESSION sql_mode='NO_AUTO_VALUE_ON_ZERO'")
            .await
            .map_err(|e| e.to_string())?;
        conn.execute("SET autocommit=0")
            .await
            .map_err(|e| e.to_string())?;

        self.import_sessions.write().insert(
            import_id.to_string(),
            Arc::new(ImportSession {
                conn: tokio::sync::Mutex::new(conn),
                max_batch_bytes,
            }),
        );
        Ok(())
    }

    async fn finish_import_session(&self, import_id: &str) -> Result<(), String> {
        use sqlx::Executor;

        let session = self.import_sessions.write().remove(import_id);
        self.running_imports.write().remove(import_id);

        if let Some(session) = session {
            let mut conn = session.conn.lock().await;
            let _ = conn.execute("COMMIT").await;
            let _ = conn.execute("SET autocommit=1").await;
            let _ = conn.execute("SET FOREIGN_KEY_CHECKS=1").await;
            let _ = conn.execute("SET SESSION sql_mode=@@GLOBAL.sql_mode").await;
        }

        Ok(())
    }

    async fn abort_import_session(&self, import_id: &str) -> Result<(), String> {
        self.import_sessions.write().remove(import_id);
        self.running_imports.write().remove(import_id);
        Ok(())
    }

    fn get_import_batch_bytes(&self, import_id: &str) -> Option<usize> {
        self.import_sessions
            .read()
            .get(import_id)
            .map(|s| s.max_batch_bytes)
    }

    async fn execute_statements(
        &self,
        database: &str,
        statements: &[String],
        import_id: Option<&str>,
    ) -> Vec<Result<(), String>> {
        use sqlx::Executor;

        let session = if let Some(import_id) = import_id {
            self.import_sessions.read().get(import_id).cloned()
        } else {
            None
        };

        let mut owned_conn = None;
        if session.is_none() {
            let mut conn = match self.pool.acquire().await {
                Ok(c) => c,
                Err(e) => return vec![Err(format!("Failed to acquire connection: {}", e))],
            };
            let use_query = format!("USE `{}`", database);
            if let Err(e) = conn.execute(use_query.as_str()).await {
                return vec![Err(format!("Failed to select database: {}", e))];
            }
            let _ = conn.execute("SET FOREIGN_KEY_CHECKS=0").await;
            let _ = conn
                .execute("SET SESSION sql_mode='NO_AUTO_VALUE_ON_ZERO'")
                .await;
            let _ = conn.execute("SET autocommit=0").await;
            owned_conn = Some(conn);
        }

        let mut results = Vec::with_capacity(statements.len());

        // Fast path: send the whole batch in one round-trip. This matters a lot
        // over SSH where latency per statement dominates large imports.
        let mut sql_block = String::new();
        for stmt in statements {
            sql_block.push_str(stmt);
            sql_block.push_str(";\n");
        }

        if let Some(session) = session {
            let mut conn = session.conn.lock().await;
            if conn.execute(sql_block.as_str()).await.is_ok() {
                results.resize_with(statements.len(), || Ok(()));
            } else if import_id
                .map(|id| !self.import_sessions.read().contains_key(id))
                .unwrap_or(false)
            {
                results.resize_with(statements.len(), || Err("Import cancelled".to_string()));
            } else {
                for stmt in statements {
                    results.push(
                        conn.execute(stmt.as_str())
                            .await
                            .map(|_| ())
                            .map_err(|e| e.to_string()),
                    );
                }
            }
        } else if let Some(mut conn) = owned_conn {
            if conn.execute(sql_block.as_str()).await.is_ok() {
                results.resize_with(statements.len(), || Ok(()));
            } else if import_id
                .map(|id| !self.import_sessions.read().contains_key(id))
                .unwrap_or(false)
            {
                results.resize_with(statements.len(), || Err("Import cancelled".to_string()));
            } else {
                // Fallback: execute one by one to preserve granular error reporting.
                for stmt in statements {
                    results.push(
                        conn.execute(stmt.as_str())
                            .await
                            .map(|_| ())
                            .map_err(|e| e.to_string()),
                    );
                }
            }

            let _ = conn.execute("COMMIT").await;
            let _ = conn.execute("SET autocommit=1").await;
            let _ = conn.execute("SET FOREIGN_KEY_CHECKS=1").await;
            let _ = conn.execute("SET SESSION sql_mode=@@GLOBAL.sql_mode").await;
        }

        results
    }
}

// --------------------------------------------------------------------------
// Tauri commands — thin wrappers over the driver
// --------------------------------------------------------------------------

fn compact_rows_for_ipc(columns: &[ColumnInfo], rows: Vec<Value>) -> Vec<Value> {
    rows.into_iter()
        .map(|row| match row {
            Value::Object(mut values) => Value::Array(
                columns
                    .iter()
                    .map(|column| values.remove(&column.name).unwrap_or(Value::Null))
                    .collect(),
            ),
            row => row,
        })
        .collect()
}

fn compact_query_result_for_ipc(mut result: QueryResult) -> QueryResult {
    result.rows = compact_rows_for_ipc(&result.columns, result.rows);
    result
}

fn retained_row_limit(column_count: usize, max_retained_cells: Option<usize>) -> usize {
    max_retained_cells
        .map(|cell_limit| {
            let column_count = column_count.max(1);
            (cell_limit / column_count).max(1)
        })
        .unwrap_or(usize::MAX)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
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
    exact_count: Option<bool>,
    keyset: Option<KeysetPage>,
) -> Result<QueryResult, String> {
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

    // Step 1: Auto-detect primary key when no sort is specified
    let effective_sort_column = if sort_column.is_none() {
        let pk_sql = format!(
            "SELECT column_name FROM information_schema.statistics WHERE table_schema='{}' AND table_name='{}' AND index_name='PRIMARY' ORDER BY seq_in_index ASC",
            database.replace('\'', "\\'"), table.replace('\'', "\\'")
        );
        let t0 = std::time::Instant::now();
        let pk_result = driver.get_primary_key_columns(&database, &table).await;
        let ms = t0.elapsed().as_millis() as u64;
        state.emit_query_log_context(
            Some(connection_id),
            Some(&database),
            &pk_sql,
            ms,
            pk_result.as_ref().err().map(|e| e.as_str()),
        );
        pk_result.ok().and_then(|cols| cols.into_iter().next())
    } else {
        sort_column
    };

    // Build WHERE clause string for query log (FilterSet is Clone)
    let (where_clause, _) = if let Some(ref f) = filters {
        crate::query_builder::build_where_clause(f)
    } else {
        (String::new(), vec![])
    };

    // order_sql starts with " ORDER BY" (space included), where_clause starts with " WHERE" or is empty
    let order_sql = match &effective_sort_column {
        Some(col) => format!(
            " ORDER BY `{}` {}",
            col,
            if sort_desc.unwrap_or(false) {
                "DESC"
            } else {
                "ASC"
            }
        ),
        None => String::new(),
    };

    // Step 3: Exact COUNT(*)
    let count_sql = format!(
        "SELECT COUNT(*) as total FROM `{}`.`{}`{}",
        database, table, where_clause
    );
    let estimated_count_sql = format!(
        "SELECT table_rows as count FROM information_schema.TABLES WHERE TABLE_SCHEMA='{}' AND TABLE_NAME='{}'",
        database.replace('\'', "\\'"), table.replace('\'', "\\'")
    );

    // Step 4: SELECT *
    let select_sql = if let Some(ref keyset) = keyset {
        format!(
            "SELECT * FROM `{}`.`{}`{}{} LIMIT {} -- keyset {} `{}`",
            database, table, where_clause, order_sql, page_size, keyset.direction, keyset.column
        )
    } else {
        format!(
            "SELECT * FROM `{}`.`{}`{}{} LIMIT {} OFFSET {}",
            database,
            table,
            where_clause,
            order_sql,
            page_size,
            page * page_size
        )
    };

    // Run the actual query (driver does COUNT + SELECT internally)
    let t0 = std::time::Instant::now();
    let result = driver
        .get_table_data(
            &database,
            &table,
            page,
            page_size,
            filters,
            effective_sort_column,
            sort_desc,
            exact_count.unwrap_or(true),
            keyset,
        )
        .await;
    let ms = t0.elapsed().as_millis() as u64;

    match &result {
        Ok(result) => {
            if let Some(timings) = &result.timings {
                let count_log_sql = if result.total_count_is_estimate {
                    &estimated_count_sql
                } else {
                    &count_sql
                };
                state.emit_query_log_context(
                    Some(connection_id),
                    Some(&database),
                    count_log_sql,
                    timings.count_ms,
                    None,
                );
                state.emit_query_log_context(
                    Some(connection_id),
                    Some(&database),
                    &select_sql,
                    timings.select_ms,
                    None,
                );
                state.emit_query_log_context(
                    Some(connection_id),
                    Some(&database),
                    &format!(
                        "-- TABLE LOAD `{}`.`{}` page={} page_size={}",
                        database, table, page, page_size
                    ),
                    timings.total_ms,
                    None,
                );
            } else {
                state.emit_query_log_context(
                    Some(connection_id),
                    Some(&database),
                    &count_sql,
                    ms,
                    None,
                );
                state.emit_query_log_context(
                    Some(connection_id),
                    Some(&database),
                    &select_sql,
                    ms,
                    None,
                );
            }
        }
        Err(e) => {
            state.emit_query_log_context(
                Some(connection_id),
                Some(&database),
                &count_sql,
                ms,
                Some(e.as_str()),
            );
            state.emit_query_log_context(
                Some(connection_id),
                Some(&database),
                &select_sql,
                ms,
                Some(e.as_str()),
            );
        }
    }

    result.map(compact_query_result_for_ipc)
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
    max_retained_cells: Option<usize>,
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
            move |columns: Option<Vec<crate::driver::ColumnInfo>>, rows: Vec<serde_json::Value>| {
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

        let result = driver
            .execute_query(
                database.as_deref(),
                &sql,
                Some(&query_id),
                on_progress,
                on_chunk,
                max_retained_cells,
            )
            .await;
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

pub async fn export_table_file(
    driver: Arc<dyn DatabaseDriver>,
    database: String,
    table: String,
    format: String,
    path: String,
    emit_progress: &(dyn Fn(usize, usize, String) + Send + Sync),
) -> Result<usize, String> {
    use std::io::{BufWriter, Write};
    use tokio::sync::mpsc;

    if format != "csv" && format != "json" && format != "sql" {
        return Err(format!("Unknown format: {}", format));
    }

    emit_progress(0, 100, format!("Streaming data from {}...", table));

    // Channel: producer streams rows, consumer writes to disk
    let (tx, mut rx) = mpsc::channel::<(Option<Vec<ColumnInfo>>, Value)>(512);

    let db_clone = database.clone();
    let table_clone = table.clone();
    let driver_clone = driver.clone();
    let stream_handle = tokio::spawn(async move {
        driver_clone
            .stream_all_rows(&db_clone, &table_clone, tx)
            .await
    });

    let file = std::fs::File::create(&path).map_err(|e| format!("Failed to create file: {}", e))?;
    let mut writer = BufWriter::new(file);
    let mut columns: Vec<ColumnInfo> = Vec::new();
    let mut row_count: usize = 0;
    let mut header_written = false;

    while let Some((col_opt, row)) = rx.recv().await {
        if let Some(cols) = col_opt {
            columns = cols;
        }

        row_count += 1;

        if row_count.is_multiple_of(5000) {
            emit_progress(50, 100, format!("Writing row {}...", row_count));
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
                    let values: Vec<String> = columns
                        .iter()
                        .map(|c| match map.get(&c.name) {
                            Some(Value::Null) | None => String::new(),
                            Some(Value::String(s)) => escape_csv(s),
                            Some(Value::Bool(b)) => b.to_string(),
                            Some(v) => v.to_string(),
                        })
                        .collect();
                    writeln!(writer, "{}", values.join(","))
                        .map_err(|e| format!("Write error: {}", e))?;
                }
            }
            "json" => {
                if !header_written {
                    writer
                        .write_all(b"[\n")
                        .map_err(|e| format!("Write error: {}", e))?;
                    header_written = true;
                } else {
                    writer
                        .write_all(b",\n")
                        .map_err(|e| format!("Write error: {}", e))?;
                }
                let row_str = serde_json::to_string(&row)
                    .map_err(|e| format!("Serialization error: {}", e))?;
                writer
                    .write_all(row_str.as_bytes())
                    .map_err(|e| format!("Write error: {}", e))?;
            }
            "sql" => {
                if !header_written {
                    writeln!(writer, "-- Export of `{}`.`{}`\n", database, table)
                        .map_err(|e| format!("Write error: {}", e))?;
                    header_written = true;
                }
                if let Value::Object(ref map) = row {
                    let col_names = columns
                        .iter()
                        .map(|c| format!("`{}`", c.name))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let values: Vec<String> = columns
                        .iter()
                        .map(|c| match map.get(&c.name) {
                            Some(Value::Null) | None => "NULL".to_string(),
                            Some(Value::String(s)) => format!("'{}'", s.replace('\'', "\\'")),
                            Some(Value::Bool(b)) => {
                                if *b {
                                    "1".to_string()
                                } else {
                                    "0".to_string()
                                }
                            }
                            Some(v) => v.to_string(),
                        })
                        .collect();
                    writeln!(
                        writer,
                        "INSERT INTO `{}` ({}) VALUES ({});",
                        table,
                        col_names,
                        values.join(", ")
                    )
                    .map_err(|e| format!("Write error: {}", e))?;
                }
            }
            _ => unreachable!(),
        }
    }

    // Close JSON array
    if format == "json" {
        if header_written {
            writer
                .write_all(b"\n]")
                .map_err(|e| format!("Write error: {}", e))?;
        } else {
            writer
                .write_all(b"[]")
                .map_err(|e| format!("Write error: {}", e))?;
        }
    }

    writer.flush().map_err(|e| format!("Write error: {}", e))?;

    // Propagate any streaming error
    stream_handle
        .await
        .map_err(|e| format!("Stream task error: {}", e))??;

    emit_progress(100, 100, "Export complete".to_string());

    Ok(row_count)
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

fn connection_allows_writes(state: &State<'_, AppState>, connection_id: Uuid) -> bool {
    let configs = state.connections_config.read();
    configs
        .get(&connection_id)
        .map(|c| c.allow_writes)
        .unwrap_or(true)
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
    if n_updates > 0 || n_deletions > 0 {
        crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id))?;
    }
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver
        .apply_table_changes(&database, &table, updates, deletions, disable_fk_checks)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    if n_updates > 0 {
        let sql = format!(
            "UPDATE `{}`.`{}` SET ... ({} row(s))",
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
            "DELETE FROM `{}`.`{}` ({} row(s))",
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
    table: String,
    values: Vec<TableChange>,
    disable_fk_checks: bool,
) -> Result<(), String> {
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id))?;
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver
        .insert_row(&database, &table, values, disable_fk_checks)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("INSERT INTO `{}`.`{}`", database, table);
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
    table: String,
    old_name: String,
    new_name: String,
    new_type: String,
) -> Result<(), String> {
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id))?;
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver
        .alter_table_column(&database, &table, &old_name, &new_name, &new_type)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    let logged_sql = result
        .as_ref()
        .ok()
        .filter(|sql| !sql.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            format!(
                "ALTER TABLE `{}`.`{}` CHANGE COLUMN `{}` `{}` {}",
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
    table: String,
    disable_fk_checks: bool,
) -> Result<(), String> {
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id))?;
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver
        .drop_table(&database, &table, disable_fk_checks)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("DROP TABLE `{}`.`{}`", database, table);
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
    tables: Vec<String>,
    disable_fk_checks: bool,
) -> Result<(), String> {
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id))?;
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver
        .drop_tables(&database, &tables, disable_fk_checks)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("DROP {} TABLE(S) FROM `{}`", tables.len(), database);
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
    table: String,
    disable_fk_checks: bool,
) -> Result<(), String> {
    crate::security::ensure_writes_allowed(connection_allows_writes(&state, connection_id))?;
    let driver = state.get_driver(&connection_id)?;
    let t0 = std::time::Instant::now();
    let result = driver
        .truncate_table(&database, &table, disable_fk_checks)
        .await;
    let ms = t0.elapsed().as_millis() as u64;
    let sql = format!("TRUNCATE TABLE `{}`.`{}`", database, table);
    state.emit_query_log_context(
        Some(connection_id),
        Some(&database),
        &sql,
        ms,
        result.as_ref().err().map(|e| e.as_str()),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn escape_csv_quotes_only_when_needed() {
        assert_eq!(escape_csv("plain"), "plain");
        assert_eq!(escape_csv("hello,world"), "\"hello,world\"");
        assert_eq!(escape_csv("hello \"world\""), "\"hello \"\"world\"\"\"");
        assert_eq!(escape_csv("hello\nworld"), "\"hello\nworld\"");
    }

    #[test]
    fn compacts_named_rows_for_the_webview_ipc_contract() {
        let columns = vec![
            ColumnInfo {
                name: "id".into(),
                type_name: "INT".into(),
            },
            ColumnInfo {
                name: "name".into(),
                type_name: "VARCHAR".into(),
            },
            ColumnInfo {
                name: "nullable".into(),
                type_name: "VARCHAR".into(),
            },
        ];
        let rows = vec![json!({ "name": "Ada", "id": 7, "nullable": null })];

        assert_eq!(
            compact_rows_for_ipc(&columns, rows),
            vec![json!([7, "Ada", null])]
        );
    }

    #[test]
    fn retained_row_limit_caps_wide_query_results_by_cells() {
        assert_eq!(retained_row_limit(200, Some(300_000)), 1_500);
        assert_eq!(retained_row_limit(20, Some(300_000)), 15_000);
        assert_eq!(retained_row_limit(0, Some(300_000)), 300_000);
        assert_eq!(retained_row_limit(200, None), usize::MAX);
        assert_eq!(retained_row_limit(200, Some(0)), 1);
    }

    #[test]
    fn sql_literal_formats_json_values_for_keyset_cursor() {
        assert_eq!(sql_literal(&Value::Null), "NULL");
        assert_eq!(sql_literal(&Value::Bool(true)), "1");
        assert_eq!(sql_literal(&Value::Bool(false)), "0");
        assert_eq!(sql_literal(&json!(42)), "42");
        assert_eq!(
            sql_literal(&Value::String("O'Reilly".into())),
            "'O\\'Reilly'"
        );
        assert_eq!(sql_literal(&Value::String("C:\\tmp".into())), "'C:\\\\tmp'");
    }

    #[test]
    fn validates_column_types_used_by_the_structure_editor() {
        for value in [
            "varchar(255)",
            "BIGINT UNSIGNED",
            "decimal(10, 2)",
            "enum('draft','published')",
            "timestamp(6)",
        ] {
            assert_eq!(validate_column_type(value).unwrap(), value);
        }
    }

    #[test]
    fn rejects_column_type_sql_injection_and_definition_attributes() {
        for value in [
            "varchar(20); DROP TABLE users",
            "int, DROP COLUMN email",
            "int NOT NULL",
            "varchar(20) COMMENT 'surprise'",
            "made_up_type",
            "enum('unfinished)",
        ] {
            assert!(validate_column_type(value).is_err(), "{value} should be rejected");
        }
    }

    #[test]
    fn quotes_mysql_identifiers_and_rejects_invalid_names() {
        assert_eq!(quote_identifier("display name").unwrap(), "`display name`");
        assert_eq!(quote_identifier("odd`name").unwrap(), "`odd``name`");
        assert!(quote_identifier("").is_err());
        assert!(quote_identifier(&"a".repeat(65)).is_err());
    }

    #[test]
    fn only_character_types_retain_charset_and_collation() {
        assert!(column_type_supports_charset("varchar(255)"));
        assert!(column_type_supports_charset("ENUM('a','b')"));
        assert!(!column_type_supports_charset("int unsigned"));
        assert!(!column_type_supports_charset("varbinary(32)"));
    }

    #[test]
    fn append_keyset_predicate_adds_where_or_and() {
        let keyset = KeysetPage {
            column: "id".to_string(),
            value: json!(100),
            direction: "next".to_string(),
        };

        assert_eq!(
            append_keyset_predicate("", &keyset, false),
            " WHERE `id` > 100"
        );
        assert_eq!(
            append_keyset_predicate(" WHERE `status` = ?", &keyset, false),
            " WHERE `status` = ? AND `id` > 100"
        );
        assert_eq!(
            append_keyset_predicate(" WHERE `status` = ?", &keyset, true),
            " WHERE `status` = ? AND `id` < 100"
        );
    }

    #[test]
    fn mysql_wkb_to_wkt_decodes_point_with_mysql_srid_prefix() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&4326u32.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1.5f64.to_le_bytes());
        bytes.extend_from_slice(&2.25f64.to_le_bytes());

        assert_eq!(mysql_wkb_to_wkt(&bytes), "POINT(1.5 2.25)");
    }

    #[test]
    fn mysql_wkb_to_wkt_falls_back_to_hex_for_unknown_geometry() {
        assert_eq!(mysql_wkb_to_wkt(&[0x01, 0x02, 0xab]), "0x0102ab");
    }
}
