use crate::connections::Connection;
use crate::driver::DatabaseDriver;
use crate::saved_queries::SavedQuery;
use crate::ssh::SshTunnel;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use uuid::Uuid;

pub struct ActiveConnection {
    pub driver: Arc<dyn DatabaseDriver>,
    pub tunnel: Option<SshTunnel>,
}

pub struct AppState {
    pub connections_config: RwLock<HashMap<Uuid, Connection>>,
    pub active_sessions: RwLock<HashMap<Uuid, ActiveConnection>>,
    pub saved_queries: RwLock<HashMap<Uuid, SavedQuery>>,
    pub canceled_imports: RwLock<HashSet<String>>,
    pub canceled_exports: RwLock<HashSet<String>>,
    pub app_handle: tauri::AppHandle,
    config_dir: PathBuf,
}

impl AppState {
    pub fn emit_query_log(&self, sql: &str, duration_ms: u64, error: Option<&str>) {
        self.emit_query_log_context(None, None, sql, duration_ms, error);
    }

    pub fn emit_query_log_context(
        &self,
        connection_id: Option<Uuid>,
        database: Option<&str>,
        sql: &str,
        duration_ms: u64,
        error: Option<&str>,
    ) {
        use serde_json::json;
        use tauri::Emitter;
        let now = chrono::Local::now();
        let _ = self.app_handle.emit(
            "query-log",
            json!({
                "connection_id": connection_id.map(|id| id.to_string()),
                "database": database,
                "sql": sql,
                "timestamp": now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                "duration_ms": duration_ms,
                "error": error,
            }),
        );
    }

    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        let config_dir = app_handle
            .path()
            .app_config_dir()
            .expect("failed to get config dir");
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).expect("failed to create config dir");
        }

        let mut connections = HashMap::new();
        let config_file = config_dir.join("connections.json");

        if config_file.exists() {
            if let Ok(content) = fs::read_to_string(&config_file) {
                if let Ok(loaded) = serde_json::from_str::<HashMap<Uuid, Connection>>(&content) {
                    connections = loaded;
                }
            }
        }

        let mut saved_queries = HashMap::new();
        let sq_file = config_dir.join("saved_queries.json");

        if sq_file.exists() {
            if let Ok(content) = fs::read_to_string(&sq_file) {
                if let Ok(loaded) = serde_json::from_str::<HashMap<Uuid, SavedQuery>>(&content) {
                    saved_queries = loaded;
                }
            }
        }

        Self {
            connections_config: RwLock::new(connections),
            active_sessions: RwLock::new(HashMap::new()),
            saved_queries: RwLock::new(saved_queries),
            canceled_imports: RwLock::new(HashSet::new()),
            canceled_exports: RwLock::new(HashSet::new()),
            app_handle: app_handle.clone(),
            config_dir,
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let config_file = self.config_dir.join("connections.json");
        let connections = self.connections_config.read();
        let content = serde_json::to_string_pretty(&*connections)
            .map_err(|e| format!("Failed to serialize connections: {}", e))?;

        fs::write(config_file, content)
            .map_err(|e| format!("Failed to write connections to disk: {}", e))?;

        Ok(())
    }

    pub fn save_queries(&self) -> Result<(), String> {
        let config_file = self.config_dir.join("saved_queries.json");
        let queries = self.saved_queries.read();
        let content = serde_json::to_string_pretty(&*queries)
            .map_err(|e| format!("Failed to serialize saved queries: {}", e))?;

        fs::write(config_file, content)
            .map_err(|e| format!("Failed to write saved queries to disk: {}", e))?;

        Ok(())
    }

    pub fn get_driver(&self, connection_id: &Uuid) -> Result<Arc<dyn DatabaseDriver>, String> {
        let sessions = self.active_sessions.read();
        Ok(sessions
            .get(connection_id)
            .ok_or("No active session found")?
            .driver
            .clone())
    }

    pub fn request_import_cancel(&self, import_id: &str) {
        self.canceled_imports.write().insert(import_id.to_string());
    }

    pub fn clear_import_cancel(&self, import_id: &str) {
        self.canceled_imports.write().remove(import_id);
    }

    pub fn is_import_canceled(&self, import_id: &str) -> bool {
        self.canceled_imports.read().contains(import_id)
    }

    pub fn request_export_cancel(&self, export_id: &str) {
        self.canceled_exports.write().insert(export_id.to_string());
    }

    pub fn clear_export_cancel(&self, export_id: &str) {
        self.canceled_exports.write().remove(export_id);
    }

    pub fn is_export_canceled(&self, export_id: &str) -> bool {
        self.canceled_exports.read().contains(export_id)
    }
}
