//! Native plain-SQL restore on a disposable, single-database session.
use super::{
    script::{Parser, Statement, MAX_STATEMENT_BYTES},
    session::cancel_backend,
    PostgreSqlDriver,
};
use crate::database::driver::*;
use async_trait::async_trait;
use sqlx::{Executor, PgConnection};
use std::{
    io::{BufRead, Read},
    time::{Duration, Instant},
};
use tokio::sync::watch;

const CANCELED: &str = "Import cancelled";
const CHUNK_BYTES: usize = 64 * 1024;

struct Registration<'a> {
    driver: &'a PostgreSqlDriver,
    id: &'a str,
}
impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.driver.running_imports.lock().remove(self.id);
    }
}

async fn canceled(rx: &mut watch::Receiver<bool>, check: &(dyn Fn() -> bool + Send + Sync)) {
    loop {
        if *rx.borrow() || check() {
            return;
        }
        tokio::select! {
            _ = rx.changed() => {},
            _ = tokio::time::sleep(Duration::from_millis(50)) => {},
        }
    }
}

#[async_trait]
impl ImportDriver for PostgreSqlDriver {
    fn handles_import_stream(&self) -> bool {
        true
    }

    async fn import_stream(
        &self,
        database: &str,
        reader: &mut (dyn BufRead + Send),
        total_bytes: usize,
        import_id: &str,
        is_canceled: &(dyn Fn() -> bool + Send + Sync),
        on_progress: &(dyn Fn(usize, usize, String) + Send + Sync),
    ) -> Result<ImportResult, String> {
        self.check_write(false)?;
        let started = Instant::now();
        let (tx, mut rx) = watch::channel(false);
        {
            let mut active = self.running_imports.lock();
            if active.contains_key(import_id) {
                return Err("Import ID is already active".into());
            }
            active.insert(import_id.into(), tx);
        }
        let _registration = Registration {
            driver: self,
            id: import_id,
        };
        let pool = tokio::select! {
            biased;
            _ = canceled(&mut rx, is_canceled) => return Err(CANCELED.into()),
            pool = self.pool_for(database) => pool?,
        };
        let mut conn = tokio::select! {
            biased;
            _ = canceled(&mut rx, is_canceled) => return Err(CANCELED.into()),
            conn = pool.acquire() => conn.map_err(|e| e.to_string())?,
        };
        conn.close_on_drop(); // SET/role/search_path/temp objects never leak.
        let (pid, backend_start): (i32, String) = sqlx::query_as(
            "SELECT pg_backend_pid(), backend_start::text FROM pg_stat_activity WHERE pid=pg_backend_pid()"
        ).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
        let mut runner = Runner {
            reader,
            total: total_bytes,
            bytes: 0,
            line: 0,
            executed: 0,
            read_time: Duration::ZERO,
            execute_time: Duration::ZERO,
            last_progress: Instant::now(),
            on_progress,
            is_canceled,
        };
        on_progress(0, total_bytes, "Importing PostgreSQL SQL…".into());
        let result = tokio::select! {
            biased;
            _ = canceled(&mut rx, is_canceled) => {
                cancel_backend(&pool, database, pid, &backend_start).await;
                Err(CANCELED.into())
            },
            result = runner.run(&mut conn) => result,
        };
        if let Err(error) = result {
            // COPY errors may leave the protocol busy. Closing the disposable
            // session rolls back without attempting to reuse that protocol.
            let _ = tokio::time::timeout(Duration::from_secs(5), conn.close()).await;
            return Err(format!(
                "{error}. Transactional changes were not committed."
            ));
        }
        if *rx.borrow() || is_canceled() {
            let _ = (&mut *conn).execute("ROLLBACK").await;
            return Err(CANCELED.into());
        }
        // Cancellation ends at the commit boundary; never report cancellation
        // after the server has committed successfully.
        on_progress(runner.bytes, total_bytes, "Committing import…".into());
        (&mut *conn).execute("COMMIT").await.map_err(|e| {
            format!("Could not confirm import commit; check the database before retrying: {e}")
        })?;
        let total_time = started.elapsed();
        on_progress(
            total_bytes,
            total_bytes,
            format!("Imported {} SQL statements", runner.executed),
        );
        Ok(ImportResult {
            executed: runner.executed,
            errors: Vec::new(),
            metrics: ImportMetrics {
                parsed_statements: runner.executed,
                compacted_statements: 0,
                executed_batches: runner.executed,
                sql_blocks: runner.executed,
                read_ms: runner.read_time.as_millis() as u64,
                process_ms: total_time
                    .saturating_sub(runner.read_time + runner.execute_time)
                    .as_millis() as u64,
                execute_ms: runner.execute_time.as_millis() as u64,
                total_ms: total_time.as_millis() as u64,
            },
        })
    }

    async fn begin_import_session(&self, _: &str, _: &str) -> Result<(), String> {
        Err("PostgreSQL requires the native streaming importer".into())
    }
    async fn abort_import_session(&self, import_id: &str) -> Result<(), String> {
        self.cancel_import(import_id).await
    }
    async fn finish_import_session(&self, _: &str) -> Result<(), String> {
        Err("PostgreSQL requires the native streaming importer".into())
    }
    async fn execute_statements(
        &self,
        _: &str,
        statements: &[String],
        _: Option<&str>,
    ) -> Vec<Result<(), String>> {
        statements
            .iter()
            .map(|_| Err("PostgreSQL requires the native streaming importer".into()))
            .collect()
    }
}

struct Runner<'a> {
    reader: &'a mut (dyn BufRead + Send),
    total: usize,
    bytes: usize,
    line: usize,
    executed: usize,
    read_time: Duration,
    execute_time: Duration,
    last_progress: Instant,
    on_progress: &'a (dyn Fn(usize, usize, String) + Send + Sync),
    is_canceled: &'a (dyn Fn() -> bool + Send + Sync),
}

impl Runner<'_> {
    fn read_line(&mut self) -> Result<Option<String>, String> {
        if (self.is_canceled)() {
            return Err(CANCELED.into());
        }
        let start = Instant::now();
        let mut bytes = Vec::new();
        let len = (&mut *self.reader)
            .take((MAX_STATEMENT_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| format!("Cannot read SQL file: {e}"))?;
        self.read_time += start.elapsed();
        if len == 0 {
            return Ok(None);
        }
        if len > MAX_STATEMENT_BYTES {
            return Err("Input line exceeds the 16 MiB import limit".into());
        }
        self.bytes += len;
        self.line += 1;
        if self.line == 1 && (bytes.starts_with(b"PGDMP") || bytes.starts_with(&[0x1f, 0x8b])) {
            return Err("Use a UTF8 plain SQL dump, not a compressed or pg_dump custom archive; convert archives with pg_restore first".into());
        }
        let line = String::from_utf8(bytes)
            .map_err(|_| format!("Input is not UTF8 near line {}", self.line))?;
        self.progress();
        Ok(Some(if self.line == 1 {
            line.trim_start_matches('\u{feff}').to_owned()
        } else {
            line
        }))
    }

    fn progress(&mut self) {
        if self.last_progress.elapsed() >= Duration::from_millis(200) {
            (self.on_progress)(
                self.bytes,
                self.total,
                format!(
                    "Importing · line {} · {} statements",
                    self.line, self.executed
                ),
            );
            self.last_progress = Instant::now();
        }
    }

    async fn run(&mut self, conn: &mut PgConnection) -> Result<(), String> {
        (&mut *conn).execute(
            "BEGIN; SET LOCAL standard_conforming_strings = on; SET LOCAL client_encoding = 'UTF8'",
        )
        .await
        .map_err(|e| e.to_string())?;
        let mut parser = Parser::new();
        let mut restrict: Option<String> = None;
        while let Some(line) = self.read_line()? {
            if parser.is_idle() && line.trim_start().starts_with('\\') {
                let words: Vec<_> = line.split_whitespace().collect();
                match words.as_slice() {
                    ["\\restrict", key] if restrict.is_none() && valid_key(key) => restrict = Some((*key).into()),
                    ["\\unrestrict", key] if restrict.as_deref() == Some(*key) => restrict = None,
                    _ => return Err(format!("Unsupported or mismatched psql command at line {}. Import a single-database plain SQL dump without --create or psql commands", self.line)),
                }
                continue;
            }
            let mut offset = 0;
            while let Some(statement) = parser.consume(&line, &mut offset, self.line)? {
                if statement.is_copy() {
                    let rest = line[offset..].trim();
                    if !rest.is_empty() && !rest.starts_with("--") {
                        return Err(format!(
                            "COPY header must end its line (line {})",
                            statement.line
                        ));
                    }
                    self.copy(conn, &statement)
                        .await
                        .map_err(|e| format!("COPY starting at line {}: {e}", statement.line))?;
                    break;
                }
                self.execute(conn, &statement).await?;
            }
            // Also allow cancellation while scanning comments/very many short lines.
            tokio::task::yield_now().await;
        }
        if let Some(statement) = parser.finish()? {
            if statement.is_copy() {
                return Err("COPY is missing its data and \\. terminator".into());
            }
            self.execute(conn, &statement).await?;
        }
        if restrict.is_some() {
            return Err("Missing matching \\unrestrict at end of dump".into());
        }
        Ok(())
    }

    async fn execute(
        &mut self,
        conn: &mut PgConnection,
        statement: &Statement,
    ) -> Result<(), String> {
        if statement.transaction_wrapper()? {
            return Ok(());
        }
        let started = Instant::now();
        let result = async {
            // Extended Parse rejects multiple server statements if lexical
            // splitting ever disagrees with the server. Execute uses simple SQL
            // so dump SET/DDL statements do not leave stale prepared plans.
            conn.describe(&statement.sql)
                .await
                .map_err(|e| e.to_string())?;
            (&mut *conn)
                .execute(statement.sql.as_str())
                .await
                .map_err(|e| e.to_string())?;
            Self::check_settings(conn).await
        }
        .await;
        self.execute_time += started.elapsed();
        result.map_err(|e| format!("SQL starting at line {}: {e}", statement.line))?;
        self.executed += 1;
        self.progress();
        Ok(())
    }

    async fn check_settings(conn: &mut PgConnection) -> Result<(), String> {
        let (strings, encoding): (String, String) = sqlx::query_as(
            "SELECT pg_catalog.current_setting('standard_conforming_strings'), pg_catalog.current_setting('client_encoding')"
        ).fetch_one(conn).await.map_err(|e| e.to_string())?;
        if strings != "on" || encoding != "UTF8" {
            return Err("Import requires standard_conforming_strings=on and client_encoding=UTF8; regenerate the dump with those settings".into());
        }
        Ok(())
    }

    async fn copy(&mut self, conn: &mut PgConnection, statement: &Statement) -> Result<(), String> {
        statement.validate_copy()?;
        let started = Instant::now();
        let read_before = self.read_time;
        conn.describe(&statement.sql)
            .await
            .map_err(|e| e.to_string())?;
        let mut copy = conn
            .copy_in_raw(&statement.sql)
            .await
            .map_err(|e| e.to_string())?;
        if !copy.is_textual() {
            let _ = copy.abort("Binary COPY is not supported").await;
            return Err(
                "Binary COPY is not supported; use text or CSV COPY in a plain SQL dump".into(),
            );
        }
        let mut chunk = Vec::with_capacity(CHUNK_BYTES);
        loop {
            let Some(line) = self.read_line()? else {
                let _ = copy.abort("Missing COPY terminator").await;
                return Err("Missing \\. terminator at end of COPY data".into());
            };
            if line.trim_end_matches(['\r', '\n']) == "\\." {
                break;
            }
            chunk.extend_from_slice(line.as_bytes());
            if chunk.len() >= CHUNK_BYTES {
                copy.send(chunk.as_slice())
                    .await
                    .map_err(|e| e.to_string())?;
                chunk.clear();
                tokio::task::yield_now().await;
            }
        }
        if !chunk.is_empty() {
            copy.send(chunk.as_slice())
                .await
                .map_err(|e| e.to_string())?;
        }
        copy.finish().await.map_err(|e| e.to_string())?;
        Self::check_settings(conn).await?;
        self.execute_time += started
            .elapsed()
            .saturating_sub(self.read_time - read_before);
        self.executed += 1;
        Ok(())
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric())
}
