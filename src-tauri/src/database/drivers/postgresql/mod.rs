mod catalog;
pub mod connection;
mod database_options;
mod editing;
mod export;
mod import;
mod queries;
mod script;
mod session;
mod sql;
mod type_name;
mod values;

use crate::database::driver::*;
use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::HashMap;
use tokio::sync::Mutex;

// Driver-private input metadata. Public/display types retain their modifiers,
// but bound values must reach assignment without an explicit narrowing cast.
struct PgColumn {
    structure: ColumnStructure,
    input_type: String,
    record_input: bool,
}
impl std::ops::Deref for PgColumn {
    type Target = ColumnStructure;
    fn deref(&self) -> &Self::Target {
        &self.structure
    }
}

pub struct PostgreSqlDriver {
    pool: PgPool,
    catalog: String,
    read_only: bool,
    // One saved connection, isolated pools per catalog. None means disconnected.
    pools: Mutex<Option<HashMap<String, PgPool>>>,
    running_imports: parking_lot::Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>,
    running_queries: parking_lot::Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>,
}

impl PostgreSqlDriver {
    pub fn new(pool: PgPool, catalog: String, read_only: bool) -> Self {
        Self {
            pools: Mutex::new(Some(HashMap::from([(catalog.clone(), pool.clone())]))),
            pool,
            catalog,
            read_only,
            running_imports: parking_lot::Mutex::new(HashMap::new()),
            running_queries: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    fn check_catalog(&self, catalog: &str) -> Result<(), String> {
        sql::quote_identifier(catalog).map(|_| ())
    }

    async fn pool_for(&self, catalog: &str) -> Result<PgPool, String> {
        self.check_catalog(catalog)?;
        {
            let state = self.pools.lock().await;
            let pools = state.as_ref().ok_or("PostgreSQL connection is closed")?;
            if let Some(pool) = pools.get(catalog) {
                return Ok(pool.clone());
            }
        }
        // Connect without holding the pools lock: this is a network round-trip
        // and must not block unrelated lookups on other already-open catalogs.
        // Inherit credentials, forwarded SSH endpoint, TLS verification, timeouts
        // and read-only policy. Never change the database of an existing session.
        let options = self
            .pool
            .connect_options()
            .as_ref()
            .clone()
            .database(catalog);
        let pool = self
            .pool
            .options()
            .clone()
            .connect_with(options)
            .await
            .map_err(|e| format!("Cannot open PostgreSQL database {catalog:?}: {e}"))?;

        let mut state = self.pools.lock().await;
        let Some(pools) = state.as_mut() else {
            drop(state);
            pool.close().await;
            return Err("PostgreSQL connection is closed".into());
        };
        if let Some(existing) = pools.get(catalog) {
            // Another task opened this catalog's pool while we were connecting.
            let existing = existing.clone();
            drop(state);
            pool.close().await;
            return Ok(existing);
        }
        pools.insert(catalog.to_owned(), pool.clone());
        Ok(pool)
    }

    fn relation(&self, table: &TableRef) -> Result<String, String> {
        self.check_catalog(&table.catalog)?;
        sql::relation(table)
    }

    fn check_write(&self, disable_fk_checks: bool) -> Result<(), String> {
        crate::security::ensure_writes_allowed(!self.read_only)?;
        if disable_fk_checks {
            return Err("Disabling foreign key checks is not supported by PostgreSQL".into());
        }
        Ok(())
    }
}

#[async_trait]
impl DatabaseDriver for PostgreSqlDriver {
    fn handles_sql_export(&self) -> bool {
        true
    }

    async fn export_sql(
        &self,
        database: &str,
        tables: &[TableRef],
        options: &SqlExportOptions,
        writer: &mut (dyn std::io::Write + Send),
        is_canceled: &(dyn Fn() -> bool + Send + Sync),
        on_progress: &(dyn Fn(usize, usize, String) + Send + Sync),
    ) -> Result<usize, String> {
        self.export_native(database, tables, options, writer, is_canceled, on_progress)
            .await
    }
    fn dialect(&self) -> &dyn crate::database::sql::SqlDialect {
        &sql::PostgreSqlDialect
    }
    fn capabilities(&self) -> crate::database::capabilities::DatabaseCapabilities {
        crate::database::capabilities::DatabaseCapabilities {
            schemas: true,
            create_database: true,
            database_collations: true,
            edit_rows: true,
            truncate_table: true,
            estimated_row_count: true,
            import_sql: true,
            export_sql: true,
            cancel_query: true,
            alter_columns: true,
            inspect_ddl: true,
            ..Default::default()
        }
    }
    async fn close(&self) {
        let pools = self.pools.lock().await.take();
        for cancel in self.running_imports.lock().values() {
            let _ = cancel.send(true);
        }
        for cancel in self.running_queries.lock().values() {
            let _ = cancel.send(true);
        }
        if let Some(pools) = pools {
            futures::future::join_all(pools.into_values().map(|pool| async move {
                pool.close().await;
            }))
            .await;
        }
    }
    async fn cancel_import(&self, import_id: &str) -> Result<(), String> {
        if let Some(cancel) = self.running_imports.lock().get(import_id) {
            let _ = cancel.send(true);
        }
        Ok(())
    }
    async fn cancel_query(&self, query_id: &str) -> Result<(), String> {
        if let Some(cancel) = self.running_queries.lock().get(query_id) {
            let _ = cancel.send(true);
        }
        Ok(())
    }
}
