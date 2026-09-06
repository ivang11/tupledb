use crate::database::driver::{ColumnInfo, DatabaseDriver, ImportResult};
use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::sync::Arc;

// --------------------------------------------------------------------------
// SQL file splitter (used by import_sql)
// --------------------------------------------------------------------------

fn statement_preview(stmt: &str) -> String {
    stmt.chars().take(60).collect()
}

#[derive(Debug)]
struct ImportBatchStatement {
    sql: String,
    preview: String,
    source_count: usize,
    compact_insert_prefix: Option<String>,
}

fn push_import_statement(
    batch: &mut Vec<ImportBatchStatement>,
    batch_bytes: &mut usize,
    stmt: String,
    max_batch_bytes: usize,
    parser: &dyn crate::database::sql::SqlImportParser,
) -> bool {
    if let Some(insert) = parser.compactable_insert(&stmt) {
        if let Some(last) = batch.last_mut() {
            if last
                .compact_insert_prefix
                .as_deref()
                .map(|prefix| prefix == &insert.prefix)
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
// Shared transfer formats and progress
// --------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct Progress {
    pub current: usize,
    pub total: usize,
    pub status: String,
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

#[allow(clippy::too_many_arguments)]
pub async fn export_database_file(
    driver: Arc<dyn DatabaseDriver>,
    database: String,
    mode: String,
    path: String,
    tables: Option<Vec<TableRef>>,
    format: &str,
    options: ExportOptions,
    emit_progress: &(dyn Fn(Progress) + Send + Sync),
    is_canceled: &(dyn Fn() -> bool + Send + Sync),
) -> Result<usize, String> {
    if !["sql", "csv", "json"].contains(&format) {
        return Err(format!("Unknown export format: {format}"));
    }
    crate::database::capabilities::require(
        format != "sql" || driver.capabilities().export_sql,
        "SQL export",
    )?;
    use std::path::Path;
    use tokio::sync::mpsc;

    let table_metadata = driver.get_tables(&database).await?;
    let view_names: HashSet<TableRef> = table_metadata
        .iter()
        .filter(|table| table.table_type.to_uppercase().contains("VIEW"))
        .map(|table| table.reference.clone())
        .collect();
    let base_table_names: HashSet<TableRef> = table_metadata
        .iter()
        .filter(|table| !table.table_type.to_uppercase().contains("VIEW"))
        .map(|table| table.reference.clone())
        .collect();

    let mut tables_to_export = match tables {
        Some(t) => t,
        None if options.include_views => table_metadata
            .iter()
            .map(|table| table.reference.clone())
            .collect(),
        None => table_metadata
            .iter()
            .filter(|table| base_table_names.contains(&table.reference))
            .map(|table| table.reference.clone())
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
                let table_clone = table.clone();
                let stream_handle =
                    tokio::spawn(
                        async move { driver_clone.stream_all_rows(&table_clone, tx).await },
                    );

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
                write!(
                    writer,
                    "\n  {}: [",
                    serde_json::to_string(&table.to_string()).map_err(|e| e.to_string())?
                )
                .map_err(|e| format!("Write error: {}", e))?;

                let (tx, mut rx) = mpsc::channel::<(Option<Vec<ColumnInfo>>, Value)>(512);
                let driver_clone = driver.clone();
                let table_clone = table.clone();
                let stream_handle =
                    tokio::spawn(
                        async move { driver_clone.stream_all_rows(&table_clone, tx).await },
                    );

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
                 {}\n\n",
                database,
                mode,
                now,
                driver.dialect().export_prologue()
            )
            .map_err(|e| format!("Failed to write file: {}", e))?;
            if options.use_transactions {
                writeln!(writer, "{}\n", driver.dialect().begin_transaction())
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
                    let create_sql = driver.get_table_ddl(table).await?;
                    if options.drop_if_exists {
                        let object_kind = if is_view { "VIEW" } else { "TABLE" };
                        writeln!(
                            writer,
                            "DROP {} IF EXISTS {};",
                            object_kind,
                            driver.dialect().quote_table_for_export(table)?
                        )
                        .map_err(|e| format!("Failed to write file: {}", e))?;
                    }
                    writeln!(writer, "{};\n", create_sql)
                        .map_err(|e| format!("Failed to write file: {}", e))?;
                }

                if include_data && !is_view {
                    let (tx, mut rx) = mpsc::channel::<(Option<Vec<ColumnInfo>>, Value)>(512);
                    let table_clone = table.clone();
                    let driver_clone = driver.clone();
                    let stream_handle = tokio::spawn(async move {
                        driver_clone.stream_all_rows(&table_clone, tx).await
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
                            let sql = driver.dialect().insert_statement(
                                table,
                                &columns,
                                &Value::Object(map),
                            )?;
                            writeln!(writer, "{sql}")
                                .map_err(|e| format!("Failed to write file: {e}"))?;
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

            writeln!(writer, "{}", driver.dialect().export_epilogue())
                .map_err(|e| format!("Failed to write file: {}", e))?;
            if options.use_transactions {
                writeln!(writer, "{}", driver.dialect().commit_transaction())
                    .map_err(|e| format!("Failed to write file: {}", e))?;
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

pub async fn import_sql_file(
    driver: Arc<dyn DatabaseDriver>,
    database: &str,
    path: &str,
    import_id: &str,
    is_canceled: &(dyn Fn() -> bool + Send + Sync),
    emit_progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<ImportResult, String> {
    use std::time::{Duration, Instant};

    crate::database::capabilities::require(driver.capabilities().import_sql, "SQL import")?;
    let mut splitter = driver.import_parser()?;
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
            if push_import_statement(&mut batch, &mut batch_bytes, $stmt, max_batch_bytes, splitter.as_ref()) {
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
        if push_import_statement(
            &mut batch,
            &mut batch_bytes,
            stmt,
            max_batch_bytes,
            splitter.as_ref(),
        ) {
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
        metrics: crate::database::driver::ImportMetrics {
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

pub fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn export_table_file(
    driver: Arc<dyn DatabaseDriver>,
    database: String,
    table: TableRef,
    format: String,
    path: String,
    emit_progress: &(dyn Fn(usize, usize, String) + Send + Sync),
) -> Result<usize, String> {
    crate::database::capabilities::require(
        format != "sql" || driver.capabilities().export_sql,
        "SQL export",
    )?;
    use std::io::{BufWriter, Write};
    use tokio::sync::mpsc;

    if format != "csv" && format != "json" && format != "sql" {
        return Err(format!("Unknown format: {}", format));
    }

    emit_progress(0, 100, format!("Streaming data from {}...", table));

    // Channel: producer streams rows, consumer writes to disk
    let (tx, mut rx) = mpsc::channel::<(Option<Vec<ColumnInfo>>, Value)>(512);

    let table_clone = table.clone();
    let driver_clone = driver.clone();
    let stream_handle =
        tokio::spawn(async move { driver_clone.stream_all_rows(&table_clone, tx).await });

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
                    let sql = driver.dialect().insert_statement(
                        &table,
                        &columns,
                        &Value::Object(map.clone()),
                    )?;
                    writeln!(writer, "{sql}").map_err(|e| format!("Write error: {e}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    fn sql_export_value(value: Option<&Value>) -> String {
        use crate::database::sql::SqlDialect;
        crate::database::drivers::mysql::sql::MySqlDialect.literal(value.unwrap_or(&Value::Null))
    }
    use crate::database::drivers::mysql::script::{parse_compactable_insert, SqlStatementSplitter};
    use crate::database::sql::SqlImportParser;
    use serde_json::json;

    #[test]
    fn escape_csv_quotes_only_when_needed() {
        assert_eq!(escape_csv("plain"), "plain");
        assert_eq!(escape_csv("hello,world"), "\"hello,world\"");
        assert_eq!(escape_csv("hello \"world\""), "\"hello \"\"world\"\"\"");
        assert_eq!(escape_csv("hello\nworld"), "\"hello\nworld\"");
    }

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
            &SqlStatementSplitter::new(),
        ));
        assert!(push_import_statement(
            &mut batch,
            &mut batch_bytes,
            "INSERT INTO users(id) VALUES (2)".to_string(),
            1024,
            &SqlStatementSplitter::new(),
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
            &SqlStatementSplitter::new(),
        );
        let merged = push_import_statement(
            &mut batch,
            &mut batch_bytes,
            "INSERT INTO roles(id) VALUES (1)".to_string(),
            1024,
            &SqlStatementSplitter::new(),
        );

        assert!(!merged);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn compaction_preserves_case_sensitive_table_names() {
        let mut batch = Vec::new();
        let mut bytes = 0;
        let parser = SqlStatementSplitter::new();
        push_import_statement(
            &mut batch,
            &mut bytes,
            "INSERT INTO Users VALUES (1)".into(),
            1024,
            &parser,
        );
        assert!(!push_import_statement(
            &mut batch,
            &mut bytes,
            "INSERT INTO users VALUES (2)".into(),
            1024,
            &parser
        ));
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn compaction_handles_unicode_identifiers_without_slicing_inside_a_character() {
        let insert = parse_compactable_insert("INSERT INTO café VALUES ('té')").unwrap();
        assert_eq!(insert.prefix, "INSERT INTO café");
        assert_eq!(insert.values, "('té')");
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
use crate::database::types::TableRef;
