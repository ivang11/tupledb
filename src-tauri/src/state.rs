use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;
use sqlx::MySqlPool;
use tauri::Manager;
use crate::connections::Connection;
use crate::ssh::SshTunnel;

pub struct ActiveConnection {
    pub pool: MySqlPool,
    pub tunnel: Option<SshTunnel>,
}

pub struct AppState {
    pub connections_config: RwLock<HashMap<Uuid, Connection>>,
    pub active_sessions: RwLock<HashMap<Uuid, ActiveConnection>>,
    config_dir: PathBuf,
}

impl AppState {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        let config_dir = app_handle.path().app_config_dir().expect("failed to get config dir");
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

        Self {
            connections_config: RwLock::new(connections),
            active_sessions: RwLock::new(HashMap::new()),
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
}
