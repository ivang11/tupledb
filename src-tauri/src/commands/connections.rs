use crate::connections::Connection;
use crate::database::capabilities::ConnectionInfo;
use crate::database::registry::{DriverDescriptor, AVAILABLE_DRIVERS};
use crate::services::connection_test::{self, ConnectionTestProgress};
use crate::services::connections as sessions;
use crate::state::AppState;
use std::sync::Arc;
use tauri::ipc::{Channel, JavaScriptChannelId};
use tauri::{State, Webview};
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct ConnectionStorageInfo {
    development: bool,
    directory: String,
}

#[tauri::command]
pub fn get_connection_storage_info(state: State<'_, AppState>) -> ConnectionStorageInfo {
    ConnectionStorageInfo {
        development: crate::connection_store::development_profile(),
        directory: state
            .connections_config
            .read()
            .directory()
            .display()
            .to_string(),
    }
}

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, String> {
    let mut store = state.connections_config.write();
    store.reload()?;
    let connections = store.data()?;
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
    connection: Connection,
) -> Result<(), String> {
    println!(
        "Saving connection: {} (Env: {:?})",
        connection.name, connection.environment
    );

    state.connections_config.write().upsert(connection)
}

#[tauri::command]
pub async fn remove_connection(state: State<'_, AppState>, id: Uuid) -> Result<(), String> {
    println!("Removing connection: {}", id);

    state.connections_config.write().remove(id)?;
    disconnect(state.clone(), id).await
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
        .data()?
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
    state
        .connections_config
        .read()
        .export(std::path::Path::new(&path))
}

#[tauri::command]
pub async fn import_connections(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    state.connections_config.write().import(content.as_bytes())
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    webview: Webview,
    mut connection: Connection,
    on_progress: Option<JavaScriptChannelId>,
) -> Result<String, String> {
    if let Some(stored) = state.connections_config.read().data()?.get(&connection.id) {
        connection.preserve_secrets_from(stored);
    }
    let channel: Option<Channel<ConnectionTestProgress>> =
        on_progress.map(|id| id.channel_on(webview));
    connection_test::test(
        &connection,
        Arc::new(move |progress| {
            if let Some(channel) = &channel {
                let _ = channel.send(progress);
            }
        }),
    )
    .await
}
