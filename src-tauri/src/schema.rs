use crate::driver::{
    ColumnInfo, ColumnStructure, DatabaseCreationOptions, DatabaseDriver, ForeignKey, ImportResult,
    Table, TableIndex,
};
use crate::state::AppState;
use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

// --------------------------------------------------------------------------
// SQL file splitter (used by import_sql)
// --------------------------------------------------------------------------

struct SqlStatementSplitter {
    current: String,
    in_single_quote: bool,
    in_double_quote: bool,
    in_backtick: bool,
    escaped: bool,
    in_line_comment: bool,
    in_block_comment: bool,
    pending_dash_comment: bool,
    pending_slash_comment: bool,
    pending_block_comment_end: bool,
}

impl SqlStatementSplitter {
    fn new() -> Self {
        Self {
            current: String::new(),
            in_single_quote: false,
            in_double_quote: false,
            in_backtick: false,
            escaped: false,
            in_line_comment: false,
            in_block_comment: false,
            pending_dash_comment: false,
            pending_slash_comment: false,
            pending_block_comment_end: false,
        }
    }

    fn push_char(&mut self, ch: char) -> Option<String> {
        if self.in_line_comment {
            if ch == '\n' {
                self.in_line_comment = false;
            }
            return None;
        }

        if self.in_block_comment {
            if self.pending_block_comment_end && ch == '/' {
                self.in_block_comment = false;
                self.pending_block_comment_end = false;
                return None;
            }
            self.pending_block_comment_end = ch == '*';
            return None;
        }

        if self.pending_dash_comment {
            if ch == '-' {
                self.pending_dash_comment = false;
                self.in_line_comment = true;
                return None;
            }
            self.current.push('-');
            self.pending_dash_comment = false;
        }

        if self.pending_slash_comment {
            if ch == '*' {
                self.pending_slash_comment = false;
                self.in_block_comment = true;
                self.pending_block_comment_end = false;
                return None;
            }
            self.current.push('/');
            self.pending_slash_comment = false;
        }

        if self.escaped {
            self.current.push(ch);
            self.escaped = false;
            return None;
        }

        match ch {
            '\\' => {
                self.escaped = true;
                self.current.push(ch);
            }
            '\'' if !self.in_double_quote && !self.in_backtick => {
                self.in_single_quote = !self.in_single_quote;
                self.current.push(ch);
            }
            '"' if !self.in_single_quote && !self.in_backtick => {
                self.in_double_quote = !self.in_double_quote;
                self.current.push(ch);
            }
            '`' if !self.in_single_quote && !self.in_double_quote => {
                self.in_backtick = !self.in_backtick;
                self.current.push(ch);
            }
            '-' if !self.in_single_quote && !self.in_double_quote && !self.in_backtick => {
                self.pending_dash_comment = true;
            }
            '/' if !self.in_single_quote && !self.in_double_quote && !self.in_backtick => {
                self.pending_slash_comment = true;
            }
            ';' if !self.in_single_quote && !self.in_double_quote && !self.in_backtick => {
                let stmt = self.current.trim().to_string();
                self.current.clear();
                if !stmt.is_empty() {
                    return Some(stmt);
                }
            }
            _ => self.current.push(ch),
        }

        None
    }

    fn finish(mut self) -> Option<String> {
        if self.pending_dash_comment {
            self.current.push('-');
        }
        if self.pending_slash_comment {
            self.current.push('/');
        }
        let stmt = self.current.trim().to_string();
        if stmt.is_empty() {
            None
        } else {
            Some(stmt)
        }
    }
}

fn statement_preview(stmt: &str) -> String {
    stmt.chars().take(60).collect()
}

#[derive(Debug)]
struct CompactableInsert {
    prefix: String,
    values: String,
}

#[derive(Debug)]
struct ImportBatchStatement {
    sql: String,
    preview: String,
    source_count: usize,
    compact_insert_prefix: Option<String>,
}

fn find_top_level_values_keyword(stmt: &str) -> Option<usize> {
    let bytes = stmt.as_bytes();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;
    let mut escaped = false;
    let mut i = 0usize;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '\'' if !in_double_quote && !in_backtick => in_single_quote = !in_single_quote,
            '"' if !in_single_quote && !in_backtick => in_double_quote = !in_double_quote,
            '`' if !in_single_quote && !in_double_quote => in_backtick = !in_backtick,
            _ => {}
        }

        if !in_single_quote
            && !in_double_quote
            && !in_backtick
            && i + 6 <= bytes.len()
            && stmt[i..i + 6].eq_ignore_ascii_case("VALUES")
        {
            let prev_ok = i == 0
                || !((bytes[i - 1] as char).is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            let next_ok = i + 6 == bytes.len()
                || !((bytes[i + 6] as char).is_ascii_alphanumeric() || bytes[i + 6] == b'_');
            if prev_ok && next_ok {
                return Some(i);
            }
        }

        i += 1;
    }

    None
}

fn parse_compactable_insert(stmt: &str) -> Option<CompactableInsert> {
    let values_idx = find_top_level_values_keyword(stmt)?;
    let prefix = stmt[..values_idx].trim_end().to_string();
    if !prefix.to_ascii_uppercase().starts_with("INSERT ") {
        return None;
    }

    let values = stmt[values_idx + "VALUES".len()..].trim().to_string();
    if !values.starts_with('(') || !values.ends_with(')') {
        return None;
    }

    Some(CompactableInsert { prefix, values })
}

fn push_import_statement(
    batch: &mut Vec<ImportBatchStatement>,
    batch_bytes: &mut usize,
    stmt: String,
    max_batch_bytes: usize,
) -> bool {
    if let Some(insert) = parse_compactable_insert(&stmt) {
        if let Some(last) = batch.last_mut() {
            if last
                .compact_insert_prefix
                .as_deref()
                .map(|prefix| prefix.eq_ignore_ascii_case(&insert.prefix))
                .unwrap_or(false)
            {
                let merged_len = last.sql.len() + 1 + insert.values.len();
                if merged_len + 2 <= max_batch_bytes {
                    last.sql.push(',');
                    last.sql.push_str(&insert.values);
                    last.source_count += 1;
                    *batch_bytes += 1 + insert.values.len();
                    return true;
                }
            }
        }

        *batch_bytes += stmt.len() + 2;
        batch.push(ImportBatchStatement {
            preview: statement_preview(&stmt),
            sql: stmt,
            source_count: 1,
            compact_insert_prefix: Some(insert.prefix),
        });
        return false;
    }

    *batch_bytes += stmt.len() + 2;
    batch.push(ImportBatchStatement {
        preview: statement_preview(&stmt),
        sql: stmt,
        source_count: 1,
        compact_insert_prefix: None,
    });
    false
}

// --------------------------------------------------------------------------
// Tauri commands
// --------------------------------------------------------------------------

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
            .and_then(|c| c.mysql.database.clone().filter(|d| !d.is_empty()))
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
        "SELECT schema_name FROM information_schema.schemata ORDER BY schema_name ASC",
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
    if name.is_empty() || name.contains('`') || name.contains(';') {
        return Err("Invalid database name".into());
    }
    let allow_writes = {
        let configs = state.connections_config.read();
        configs
            .get(&connection_id)
            .map(|c| c.allow_writes)
            .unwrap_or(true)
    };
    crate::security::ensure_writes_allowed(allow_writes)?;
    let driver = state.get_driver(&connection_id)?;
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
    if name.is_empty() || name.contains('`') || name.contains(';') {
        return Err("Invalid database name".into());
    }
    let allow_writes = {
        let configs = state.connections_config.read();
        configs
            .get(&connection_id)
            .map(|c| c.allow_writes)
            .unwrap_or(true)
    };
    crate::security::ensure_writes_allowed(allow_writes)?;
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
    table: String,
) -> Result<Vec<ColumnStructure>, String> {
    let driver = state.get_driver(&connection_id)?;
    let sql = format!("SHOW COLUMNS FROM `{}`.`{}`", database, table);
    let t0 = std::time::Instant::now();
    let result = driver.get_table_structure(&database, &table).await;
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
    table: String,
) -> Result<Vec<ForeignKey>, String> {
    let driver = state.get_driver(&connection_id)?;
    let sql = format!(
        "SELECT COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME \
         FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE \
         WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' AND REFERENCED_TABLE_NAME IS NOT NULL",
        database, table
    );
    let t0 = std::time::Instant::now();
    let result = driver.get_foreign_keys(&database, &table).await;
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
    table: String,
) -> Result<Vec<TableIndex>, String> {
    let driver = state.get_driver(&connection_id)?;
    let sql = format!("SHOW INDEX FROM `{}`.`{}`", database, table);
    let t0 = std::time::Instant::now();
    let result = driver.get_table_indexes(&database, &table).await;
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
    table: String,
) -> Result<String, String> {
    let driver = state.get_driver(&connection_id)?;
    let sql = format!("SHOW CREATE TABLE `{}`.`{}`", database, table);
    let t0 = std::time::Instant::now();
    let result = driver.get_table_ddl(&database, &table).await;
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

#[derive(Clone, Serialize, Deserialize)]
pub struct Progress {
    pub current: usize,
    pub total: usize,
    pub status: String,
}

fn sql_export_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::Null) | None => "NULL".to_string(),
        Some(Value::String(s)) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        Some(Value::Bool(b)) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Some(v) => v.to_string(),
    }
}

fn csv_export_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::Null) | None => String::new(),
        Some(Value::String(s)) => {
            if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        }
        Some(Value::Bool(b)) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Some(v) => v.to_string(),
    }
}

#[derive(Clone, Copy)]
pub struct ExportOptions {
    pub drop_if_exists: bool,
    pub include_views: bool,
    pub use_transactions: bool,
    pub compress_gzip: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            drop_if_exists: true,
            include_views: true,
            use_transactions: true,
            compress_gzip: false,
        }
    }
}

enum ExportWriter {
    Plain(BufWriter<File>),
    Gzip(GzEncoder<BufWriter<File>>),
}

impl ExportWriter {
    fn new(path: &str, compress_gzip: bool) -> Result<Self, String> {
        let file = File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;
        let writer = BufWriter::new(file);
        if compress_gzip {
            Ok(Self::Gzip(GzEncoder::new(writer, Compression::default())))
        } else {
            Ok(Self::Plain(writer))
        }
    }

    fn finish(self) -> Result<(), String> {
        match self {
            Self::Plain(mut writer) => writer.flush().map_err(|e| format!("Flush error: {}", e)),
            Self::Gzip(writer) => {
                let mut writer = writer
                    .finish()
                    .map_err(|e| format!("Gzip finish error: {}", e))?;
                writer.flush().map_err(|e| format!("Flush error: {}", e))
            }
        }
    }
}

impl Write for ExportWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(writer) => writer.write(buf),
            Self::Gzip(writer) => writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(writer) => writer.flush(),
            Self::Gzip(writer) => writer.flush(),
        }
    }
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
    tables: Option<Vec<String>>,
    export_id: Option<String>,
    format: Option<String>,
    drop_if_exists: Option<bool>,
    include_views: Option<bool>,
    use_transactions: Option<bool>,
    compress_gzip: Option<bool>,
) -> Result<usize, String> {
    use tauri::Emitter;

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

#[allow(clippy::too_many_arguments)]
pub async fn export_database_file(
    driver: Arc<dyn DatabaseDriver>,
    database: String,
    mode: String,
    path: String,
    tables: Option<Vec<String>>,
    format: &str,
    options: ExportOptions,
    emit_progress: &(dyn Fn(Progress) + Send + Sync),
    is_canceled: &(dyn Fn() -> bool + Send + Sync),
) -> Result<usize, String> {
    use std::path::Path;
    use tokio::sync::mpsc;

    let table_metadata = driver.get_tables(&database).await?;
    let view_names: HashSet<String> = table_metadata
        .iter()
        .filter(|table| table.table_type.to_uppercase().contains("VIEW"))
        .map(|table| table.name.clone())
        .collect();
    let base_table_names: HashSet<String> = table_metadata
        .iter()
        .filter(|table| !table.table_type.to_uppercase().contains("VIEW"))
        .map(|table| table.name.clone())
        .collect();

    let mut tables_to_export = match tables {
        Some(t) => t,
        None if options.include_views => table_metadata
            .iter()
            .map(|table| table.name.clone())
            .collect(),
        None => table_metadata
            .iter()
            .filter(|table| base_table_names.contains(&table.name))
            .map(|table| table.name.clone())
            .collect(),
    };
    if !options.include_views {
        tables_to_export.retain(|table| base_table_names.contains(table));
    }

    let total_tables = tables_to_export.len();
    let mut total_rows = 0usize;

    match format {
        // ── CSV: one file per table ──────────────────────────────────────────
        "csv" => {
            let base_path = Path::new(&path);
            let stem = base_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let dir = base_path.parent().unwrap_or(Path::new("."));

            for (i, table) in tables_to_export.iter().enumerate() {
                if is_canceled() {
                    return Err("Export cancelled".to_string());
                }

                emit_progress(Progress {
                    current: i,
                    total: total_tables,
                    status: format!("Exporting {}", table),
                });

                let table_path = dir.join(format!(
                    "{}_{}.csv{}",
                    stem,
                    table,
                    if options.compress_gzip { ".gz" } else { "" }
                ));
                let mut writer = ExportWriter::new(
                    table_path.to_str().ok_or_else(|| {
                        format!("Export path is not valid UTF-8: {}", table_path.display())
                    })?,
                    options.compress_gzip,
                )
                .map_err(|e| format!("Failed to create {}: {}", table_path.display(), e))?;

                let (tx, mut rx) = mpsc::channel::<(Option<Vec<ColumnInfo>>, Value)>(512);
                let driver_clone = driver.clone();
                let db_clone = database.clone();
                let table_clone = table.clone();
                let stream_handle = tokio::spawn(async move {
                    driver_clone
                        .stream_all_rows(&db_clone, &table_clone, tx)
                        .await
                });

                let mut columns: Vec<ColumnInfo> = Vec::new();
                let mut header_written = false;
                let mut table_rows = 0usize;

                while let Some((col_opt, row)) = rx.recv().await {
                    if is_canceled() {
                        stream_handle.abort();
                        return Err("Export cancelled".to_string());
                    }
                    if let Some(cols) = col_opt {
                        columns = cols;
                    }

                    if !header_written && !columns.is_empty() {
                        let header: Vec<String> = columns
                            .iter()
                            .map(|c| csv_export_value(Some(&Value::String(c.name.clone()))))
                            .collect();
                        writeln!(writer, "{}", header.join(","))
                            .map_err(|e| format!("Write error: {}", e))?;
                        header_written = true;
                    }

                    if let Value::Object(ref map) = row {
                        let values: Vec<String> = columns
                            .iter()
                            .map(|c| csv_export_value(map.get(&c.name)))
                            .collect();
                        writeln!(writer, "{}", values.join(","))
                            .map_err(|e| format!("Write error: {}", e))?;
                        table_rows += 1;
                        total_rows += 1;
                        if table_rows.is_multiple_of(5000) {
                            emit_progress(Progress {
                                current: i,
                                total: total_tables,
                                status: format!("Exporting {}: {} rows", table, table_rows),
                            });
                        }
                    }
                }

                stream_handle
                    .await
                    .map_err(|e| format!("Stream failed: {}", e))??;
                writer.finish()?;
            }
        }

        // ── JSON: single file, object keyed by table name ────────────────────
        "json" => {
            let mut writer = ExportWriter::new(&path, options.compress_gzip)?;
            write!(writer, "{{").map_err(|e| format!("Write error: {}", e))?;

            for (i, table) in tables_to_export.iter().enumerate() {
                if is_canceled() {
                    return Err("Export cancelled".to_string());
                }

                emit_progress(Progress {
                    current: i,
                    total: total_tables,
                    status: format!("Exporting {}", table),
                });

                if i > 0 {
                    write!(writer, ",").map_err(|e| format!("Write error: {}", e))?;
                }
                write!(writer, "\n  \"{}\": [", table)
                    .map_err(|e| format!("Write error: {}", e))?;

                let (tx, mut rx) = mpsc::channel::<(Option<Vec<ColumnInfo>>, Value)>(512);
                let driver_clone = driver.clone();
                let db_clone = database.clone();
                let table_clone = table.clone();
                let stream_handle = tokio::spawn(async move {
                    driver_clone
                        .stream_all_rows(&db_clone, &table_clone, tx)
                        .await
                });

                let mut first_row = true;
                let mut table_rows = 0usize;

                while let Some((_, row)) = rx.recv().await {
                    if is_canceled() {
                        stream_handle.abort();
                        return Err("Export cancelled".to_string());
                    }
                    if let Value::Object(_) = &row {
                        if !first_row {
                            write!(writer, ",").map_err(|e| format!("Write error: {}", e))?;
                        }
                        write!(
                            writer,
                            "\n    {}",
                            serde_json::to_string(&row).unwrap_or_default()
                        )
                        .map_err(|e| format!("Write error: {}", e))?;
                        first_row = false;
                        table_rows += 1;
                        total_rows += 1;
                        if table_rows.is_multiple_of(5000) {
                            emit_progress(Progress {
                                current: i,
                                total: total_tables,
                                status: format!("Exporting {}: {} rows", table, table_rows),
                            });
                        }
                    }
                }

                stream_handle
                    .await
                    .map_err(|e| format!("Stream failed: {}", e))??;
                write!(writer, "\n  ]").map_err(|e| format!("Write error: {}", e))?;
            }

            write!(writer, "\n}}\n").map_err(|e| format!("Write error: {}", e))?;
            writer.finish()?;
        }

        // ── SQL (default) ────────────────────────────────────────────────────
        _ => {
            let include_structure = mode == "structure" || mode == "full";
            let include_data = mode == "data" || mode == "full";
            let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

            let mut writer = ExportWriter::new(&path, options.compress_gzip)?;

            write!(
                writer,
                "-- TupleDB Export\n-- Database: `{}`\n-- Mode: {}\n-- Generated: {}\n\
                 -- --------------------------------------------------------\n\n\
                 SET FOREIGN_KEY_CHECKS=0;\n\n",
                database, mode, now
            )
            .map_err(|e| format!("Failed to write file: {}", e))?;
            if options.use_transactions {
                writeln!(writer, "START TRANSACTION;\n")
                    .map_err(|e| format!("Failed to write file: {}", e))?;
            }

            for (i, table) in tables_to_export.iter().enumerate() {
                if is_canceled() {
                    return Err("Export cancelled".to_string());
                }
                let is_view = view_names.contains(table);

                emit_progress(Progress {
                    current: i,
                    total: total_tables,
                    status: format!("Exporting table {} of {} ({})", i + 1, total_tables, table),
                });

                write!(
                    writer,
                    "-- --------------------------------------------------------\n\
                     -- Table: `{}`\n\
                     -- --------------------------------------------------------\n\n",
                    table
                )
                .map_err(|e| format!("Failed to write file: {}", e))?;

                if include_structure {
                    let create_sql = driver.get_table_ddl(&database, table).await?;
                    if options.drop_if_exists {
                        let object_kind = if is_view { "VIEW" } else { "TABLE" };
                        writeln!(writer, "DROP {} IF EXISTS `{}`;", object_kind, table)
                            .map_err(|e| format!("Failed to write file: {}", e))?;
                    }
                    writeln!(writer, "{};\n", create_sql)
                        .map_err(|e| format!("Failed to write file: {}", e))?;
                }

                if include_data && !is_view {
                    let (tx, mut rx) = mpsc::channel::<(Option<Vec<ColumnInfo>>, Value)>(512);
                    let db_clone = database.clone();
                    let table_clone = table.clone();
                    let driver_clone = driver.clone();
                    let stream_handle = tokio::spawn(async move {
                        driver_clone
                            .stream_all_rows(&db_clone, &table_clone, tx)
                            .await
                    });

                    let mut columns: Vec<ColumnInfo> = Vec::new();
                    let mut table_rows = 0usize;

                    while let Some((col_opt, row)) = rx.recv().await {
                        if is_canceled() {
                            stream_handle.abort();
                            return Err("Export cancelled".to_string());
                        }
                        if let Some(cols) = col_opt {
                            columns = cols;
                        }

                        if let Value::Object(map) = row {
                            let col_names = columns
                                .iter()
                                .map(|c| format!("`{}`", c.name))
                                .collect::<Vec<_>>()
                                .join(", ");
                            let values = columns
                                .iter()
                                .map(|c| sql_export_value(map.get(&c.name)))
                                .collect::<Vec<_>>()
                                .join(", ");
                            writeln!(
                                writer,
                                "INSERT INTO `{}` ({}) VALUES ({});",
                                table, col_names, values
                            )
                            .map_err(|e| format!("Failed to write file: {}", e))?;
                            table_rows += 1;
                            total_rows += 1;
                            if table_rows.is_multiple_of(5000) {
                                emit_progress(Progress {
                                    current: i,
                                    total: total_tables,
                                    status: format!(
                                        "Exporting {}: {} rows written",
                                        table, table_rows
                                    ),
                                });
                            }
                        }
                    }

                    stream_handle
                        .await
                        .map_err(|e| format!("Export stream task failed: {}", e))??;
                    if table_rows > 0 {
                        writeln!(writer).map_err(|e| format!("Failed to write file: {}", e))?;
                    }
                }
            }

            writeln!(writer, "SET FOREIGN_KEY_CHECKS=1;")
                .map_err(|e| format!("Failed to write file: {}", e))?;
            if options.use_transactions {
                writeln!(writer, "COMMIT;").map_err(|e| format!("Failed to write file: {}", e))?;
            }
            writer.finish()?;
        }
    }

    emit_progress(Progress {
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

pub async fn import_sql_file(
    driver: Arc<dyn DatabaseDriver>,
    database: &str,
    path: &str,
    import_id: &str,
    is_canceled: &(dyn Fn() -> bool + Send + Sync),
    emit_progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<ImportResult, String> {
    use std::time::{Duration, Instant};

    let file = File::open(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let total_bytes = file.metadata().map(|m| m.len() as usize).unwrap_or(0);
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    driver.begin_import_session(database, import_id).await?;

    emit_progress(Progress {
        current: 0,
        total: total_bytes,
        status: "Reading SQL dump...".to_string(),
    });

    // Large batches reduce round-trips a lot over SSH, but we cap both by
    // statement count and by total SQL bytes to avoid pathological giant blocks.
    const MAX_BATCH_STATEMENTS: usize = 5_000;
    let max_batch_bytes = driver
        .get_import_batch_bytes(import_id)
        .unwrap_or(4 * 1024 * 1024);
    let mut splitter = SqlStatementSplitter::new();
    let mut batch: Vec<ImportBatchStatement> = Vec::with_capacity(MAX_BATCH_STATEMENTS.min(1024));
    let mut batch_bytes = 0usize;
    let mut line = String::new();
    let mut bytes_read = 0usize;
    let mut executed = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut parsed_statements = 0usize;
    let mut compacted_statements = 0usize;
    let mut executed_batches = 0usize;
    let mut sql_blocks = 0usize;
    let mut read_time = Duration::ZERO;
    let mut process_time = Duration::ZERO;
    let mut execute_time = Duration::ZERO;
    let import_started = Instant::now();

    macro_rules! queue_import_statement {
        ($stmt:expr, $process_started:ident) => {{
            parsed_statements += 1;
            if push_import_statement(&mut batch, &mut batch_bytes, $stmt, max_batch_bytes) {
                compacted_statements += 1;
            }
            if batch.len() >= MAX_BATCH_STATEMENTS || batch_bytes >= max_batch_bytes {
                let status = if errors.is_empty() {
                    format!(
                        "Executing batch... {} statements parsed, {} queued, {:.1} MB",
                        parsed_statements,
                        batch.len(),
                        batch_bytes as f64 / (1024.0 * 1024.0),
                    )
                } else {
                    format!(
                        "Executing batch... {} statements parsed, {} queued, {:.1} MB, {} errors",
                        parsed_statements,
                        batch.len(),
                        batch_bytes as f64 / (1024.0 * 1024.0),
                        errors.len(),
                    )
                };
                emit_progress(Progress {
                    current: bytes_read,
                    total: total_bytes,
                    status,
                });

                process_time += $process_started.elapsed();
                let batch_sql: Vec<String> = batch.iter().map(|stmt| stmt.sql.clone()).collect();
                let batch_started = Instant::now();
                let batch_results = driver
                    .execute_statements(database, &batch_sql, Some(import_id))
                    .await;
                execute_time += batch_started.elapsed();
                executed_batches += 1;
                sql_blocks += batch.len();
                for (result, stmt) in batch_results.into_iter().zip(batch.iter()) {
                    match result {
                        Ok(()) => executed += stmt.source_count,
                        Err(e) => {
                            let prefix = if stmt.source_count > 1 {
                                format!("{} [x{}]", stmt.preview, stmt.source_count)
                            } else {
                                stmt.preview.clone()
                            };
                            errors.push(format!("{}: {}", prefix, e));
                        }
                    }
                }
                if is_canceled() {
                    emit_progress(Progress {
                        current: bytes_read,
                        total: total_bytes,
                        status: "Import cancelled".to_string(),
                    });
                    let _ = driver.finish_import_session(import_id).await;
                    return Err("Import cancelled".to_string());
                }
                batch.clear();
                batch_bytes = 0;
                $process_started = Instant::now();
                let status = if errors.is_empty() {
                    format!("Executing... {} statements parsed", parsed_statements)
                } else {
                    format!(
                        "Executing... {} statements parsed, {} errors",
                        parsed_statements,
                        errors.len(),
                    )
                };
                emit_progress(Progress {
                    current: bytes_read,
                    total: total_bytes,
                    status,
                });
            }
        }};
    }

    loop {
        if is_canceled() {
            emit_progress(Progress {
                current: bytes_read,
                total: total_bytes,
                status: "Import cancelled".to_string(),
            });
            let _ = driver.finish_import_session(import_id).await;
            return Err("Import cancelled".to_string());
        }

        line.clear();
        let read_started = Instant::now();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| format!("Failed while reading file: {}", e))?;
        read_time += read_started.elapsed();
        if read == 0 {
            break;
        }
        bytes_read = (bytes_read + read).min(total_bytes);

        let mut process_started = Instant::now();
        for ch in line.chars() {
            if let Some(stmt) = splitter.push_char(ch) {
                queue_import_statement!(stmt, process_started);
            }
        }
        process_time += process_started.elapsed();

        if bytes_read % (8 * 1024 * 1024) < read {
            emit_progress(Progress {
                current: bytes_read,
                total: total_bytes,
                status: format!("Reading... {} statements parsed", parsed_statements),
            });
        }
    }

    if let Some(stmt) = splitter.finish() {
        parsed_statements += 1;
        let process_started = Instant::now();
        if push_import_statement(&mut batch, &mut batch_bytes, stmt, max_batch_bytes) {
            compacted_statements += 1;
        }
        process_time += process_started.elapsed();
    }

    if !batch.is_empty() {
        let status = if errors.is_empty() {
            format!(
                "Executing final batch... {} statements parsed, {} queued, {:.1} MB",
                parsed_statements,
                batch.len(),
                batch_bytes as f64 / (1024.0 * 1024.0),
            )
        } else {
            format!(
                "Executing final batch... {} statements parsed, {} queued, {:.1} MB, {} errors",
                parsed_statements,
                batch.len(),
                batch_bytes as f64 / (1024.0 * 1024.0),
                errors.len(),
            )
        };
        emit_progress(Progress {
            current: bytes_read,
            total: total_bytes,
            status,
        });

        let batch_sql: Vec<String> = batch.iter().map(|stmt| stmt.sql.clone()).collect();
        let batch_started = Instant::now();
        let batch_results = driver
            .execute_statements(database, &batch_sql, Some(import_id))
            .await;
        execute_time += batch_started.elapsed();
        executed_batches += 1;
        sql_blocks += batch.len();
        for (result, stmt) in batch_results.into_iter().zip(batch.iter()) {
            match result {
                Ok(()) => executed += stmt.source_count,
                Err(e) => {
                    let prefix = if stmt.source_count > 1 {
                        format!("{} [x{}]", stmt.preview, stmt.source_count)
                    } else {
                        stmt.preview.clone()
                    };
                    errors.push(format!("{}: {}", prefix, e));
                }
            }
        }
        if is_canceled() {
            emit_progress(Progress {
                current: bytes_read,
                total: total_bytes,
                status: "Import cancelled".to_string(),
            });
            let _ = driver.finish_import_session(import_id).await;
            return Err("Import cancelled".to_string());
        }
    }

    let final_status = if errors.is_empty() {
        format!("Import complete. {} statements executed.", executed)
    } else {
        format!(
            "Import complete. {} statements executed, {} errors.",
            executed,
            errors.len(),
        )
    };
    emit_progress(Progress {
        current: total_bytes,
        total: total_bytes,
        status: final_status,
    });

    let _ = driver.finish_import_session(import_id).await;

    Ok(ImportResult {
        executed,
        errors,
        metrics: crate::driver::ImportMetrics {
            parsed_statements,
            compacted_statements,
            executed_batches,
            sql_blocks,
            read_ms: read_time.as_millis() as u64,
            process_ms: process_time.as_millis() as u64,
            execute_ms: execute_time.as_millis() as u64,
            total_ms: import_started.elapsed().as_millis() as u64,
        },
    })
}

#[tauri::command]
pub async fn cancel_import(
    state: State<'_, AppState>,
    connection_id: Uuid,
    import_id: String,
) -> Result<(), String> {
    state.request_import_cancel(&import_id);
    let driver = state.get_driver(&connection_id)?;
    if let Some(thread_id) = driver.get_thread_id_for_import(&import_id) {
        let _ = driver.abort_import_session(&import_id).await;
        let _ = driver.kill_connection(thread_id).await;
    } else {
        let _ = driver.abort_import_session(&import_id).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn split_sql(sql: &str) -> Vec<String> {
        let mut splitter = SqlStatementSplitter::new();
        let mut statements = Vec::new();
        for ch in sql.chars() {
            if let Some(stmt) = splitter.push_char(ch) {
                statements.push(stmt);
            }
        }
        if let Some(stmt) = splitter.finish() {
            statements.push(stmt);
        }
        statements
    }

    #[test]
    fn statement_preview_truncates_to_sixty_chars() {
        let stmt = "x".repeat(80);

        assert_eq!(statement_preview(&stmt).len(), 60);
    }

    #[test]
    fn sql_export_value_escapes_strings_and_formats_primitives() {
        assert_eq!(sql_export_value(None), "NULL");
        assert_eq!(sql_export_value(Some(&Value::Null)), "NULL");
        assert_eq!(sql_export_value(Some(&Value::Bool(true))), "1");
        assert_eq!(sql_export_value(Some(&Value::Bool(false))), "0");
        assert_eq!(sql_export_value(Some(&json!(123))), "123");
        assert_eq!(
            sql_export_value(Some(&Value::String("O'Reilly\\books".into()))),
            "'O\\'Reilly\\\\books'"
        );
    }

    #[test]
    fn parse_compactable_insert_extracts_prefix_and_values() {
        let insert =
            parse_compactable_insert("INSERT INTO `users` (`id`, `name`) VALUES (1, 'Ada')")
                .expect("compactable insert");

        assert_eq!(insert.prefix, "INSERT INTO `users` (`id`, `name`)");
        assert_eq!(insert.values, "(1, 'Ada')");
    }

    #[test]
    fn parse_compactable_insert_ignores_values_inside_strings() {
        let insert =
            parse_compactable_insert("INSERT INTO logs(message) VALUES ('literal VALUES text')")
                .expect("compactable insert");

        assert_eq!(insert.prefix, "INSERT INTO logs(message)");
        assert_eq!(insert.values, "('literal VALUES text')");
    }

    #[test]
    fn parse_compactable_insert_rejects_non_insert_or_malformed_values() {
        assert!(parse_compactable_insert("UPDATE users SET name = 'Ada'").is_none());
        assert!(parse_compactable_insert("INSERT INTO users VALUES 1, 2").is_none());
    }

    #[test]
    fn push_import_statement_merges_compatible_inserts() {
        let mut batch = Vec::new();
        let mut batch_bytes = 0;

        assert!(!push_import_statement(
            &mut batch,
            &mut batch_bytes,
            "INSERT INTO users(id) VALUES (1)".to_string(),
            1024,
        ));
        assert!(push_import_statement(
            &mut batch,
            &mut batch_bytes,
            "INSERT INTO users(id) VALUES (2)".to_string(),
            1024,
        ));

        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].source_count, 2);
        assert_eq!(batch[0].sql, "INSERT INTO users(id) VALUES (1),(2)");
        assert!(batch_bytes > 0);
    }

    #[test]
    fn push_import_statement_does_not_merge_different_insert_prefixes() {
        let mut batch = Vec::new();
        let mut batch_bytes = 0;

        push_import_statement(
            &mut batch,
            &mut batch_bytes,
            "INSERT INTO users(id) VALUES (1)".to_string(),
            1024,
        );
        let merged = push_import_statement(
            &mut batch,
            &mut batch_bytes,
            "INSERT INTO roles(id) VALUES (1)".to_string(),
            1024,
        );

        assert!(!merged);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn sql_splitter_splits_basic_statements_and_flushes_final_statement() {
        let statements = split_sql("CREATE TABLE users(id INT); INSERT INTO users VALUES (1)");

        assert_eq!(
            statements,
            vec![
                "CREATE TABLE users(id INT)".to_string(),
                "INSERT INTO users VALUES (1)".to_string(),
            ]
        );
    }

    #[test]
    fn sql_splitter_keeps_semicolons_inside_quotes() {
        let statements = split_sql(
            "INSERT INTO logs(message) VALUES ('hello; world', \"double; quote\"); SELECT 1;",
        );

        assert_eq!(
            statements,
            vec![
                "INSERT INTO logs(message) VALUES ('hello; world', \"double; quote\")".to_string(),
                "SELECT 1".to_string(),
            ]
        );
    }

    #[test]
    fn sql_splitter_keeps_semicolons_inside_backticks() {
        let statements = split_sql(
            "CREATE TABLE `weird;name` (`semi;col` INT); SELECT `semi;col` FROM `weird;name`;",
        );

        assert_eq!(
            statements,
            vec![
                "CREATE TABLE `weird;name` (`semi;col` INT)".to_string(),
                "SELECT `semi;col` FROM `weird;name`".to_string(),
            ]
        );
    }

    #[test]
    fn sql_splitter_ignores_line_and_block_comments() {
        let statements = split_sql(
            "-- ignore; this line\nSELECT 1; /* ignore; block */ INSERT INTO t VALUES (2);",
        );

        assert_eq!(
            statements,
            vec![
                "SELECT 1".to_string(),
                "INSERT INTO t VALUES (2)".to_string(),
            ]
        );
    }

    #[test]
    fn sql_splitter_keeps_comment_like_tokens_inside_strings() {
        let statements = split_sql(
            "INSERT INTO logs(message) VALUES ('not -- comment; still string', 'not /* block; */ either');",
        );

        assert_eq!(
            statements,
            vec![
                "INSERT INTO logs(message) VALUES ('not -- comment; still string', 'not /* block; */ either')".to_string(),
            ]
        );
    }

    #[test]
    fn sql_splitter_preserves_pending_dash_or_slash_when_not_comment() {
        let statements = split_sql("SELECT 5-2; SELECT 6/3;");

        assert_eq!(
            statements,
            vec!["SELECT 5-2".to_string(), "SELECT 6/3".to_string()]
        );
    }
}
