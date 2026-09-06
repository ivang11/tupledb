use crate::connections::Connection;
use crate::database::capabilities::ConnectionInfo;
use crate::database::registry::{DriverDescriptor, AVAILABLE_DRIVERS};
use crate::services::connections as sessions;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, String> {
    let connections = state.connections_config.read();
    // Strip passwords before sending to frontend
    Ok(connections
        .values()
        .map(|c| {
            let mut c = c.clone();
            c.database.strip_password();
            if let Some(ssh) = &mut c.ssh {
                match &mut ssh.auth {
                    crate::connections::SshAuth::Password { password } => {
                        *password = String::new();
                    }
                    crate::connections::SshAuth::Key { passphrase, .. } => {
                        *passphrase = None;
                    }
                }
            }
            c
        })
        .collect())
}

#[tauri::command]
pub async fn add_connection(
    state: State<'_, AppState>,
    mut connection: Connection,
) -> Result<(), String> {
    println!(
        "Saving connection: {} (Env: {:?})",
        connection.name, connection.environment
    );

    // If editing and a password field is empty, preserve the existing stored password
    {
        let existing = state.connections_config.read();
        if let Some(existing_conn) = existing.get(&connection.id) {
            connection.preserve_secrets_from(existing_conn);
        }
    }

    let mut connections = state.connections_config.write();
    connections.insert(connection.id, connection);
    drop(connections);
    state.save()
}

#[tauri::command]
pub async fn remove_connection(state: State<'_, AppState>, id: Uuid) -> Result<(), String> {
    println!("Removing connection: {}", id);

    disconnect(state.clone(), id).await?;

    let mut connections = state.connections_config.write();
    connections.remove(&id);
    drop(connections);

    state.save()
}

#[tauri::command]
pub fn get_available_drivers() -> &'static [DriverDescriptor] {
    AVAILABLE_DRIVERS
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    connection: Connection,
) -> Result<ConnectionInfo, String> {
    let connection = state
        .connections_config
        .read()
        .get(&connection.id)
        .cloned()
        .unwrap_or(connection);
    let (session, info) = sessions::open(&connection).await?;
    let previous = state.active_sessions.write().insert(connection.id, session);
    if let Some(previous) = previous {
        sessions::close(previous).await;
    }
    Ok(info)
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>, connection_id: Uuid) -> Result<(), String> {
    let session = state.active_sessions.write().remove(&connection_id);
    if let Some(session) = session {
        sessions::close(session).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn export_connections(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let connections = state.connections_config.read();
    let content = serde_json::to_string_pretty(&*connections)
        .map_err(|e| format!("Failed to serialize connections: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn import_connections(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    let imported: std::collections::HashMap<Uuid, Connection> =
        serde_json::from_str(&content).map_err(|e| format!("Invalid connections file: {}", e))?;
    let count = imported.len();
    let mut connections = state.connections_config.write();
    for (id, conn) in imported {
        connections.insert(id, conn);
    }
    drop(connections);
    state.save()?;
    Ok(count)
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    mut connection: Connection,
) -> Result<String, String> {
    if let Some(stored) = state.connections_config.read().get(&connection.id) {
        connection.preserve_secrets_from(stored);
    }
    let (session, _) = sessions::open(&connection).await?;
    sessions::close(session).await;
    Ok("Connected successfully".into())
}
