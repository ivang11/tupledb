use super::*;

#[async_trait]
impl ImportDriver for MySqlDriver {
    fn import_parser(&self) -> Result<Box<dyn crate::database::sql::SqlImportParser>, String> {
        Ok(Box::new(script::SqlStatementSplitter::new()))
    }
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
