use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use crate::state::AppState;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedQuery {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub sql: String,
    pub connection_id: Option<Uuid>,
    pub database: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub async fn get_saved_queries(state: State<'_, AppState>) -> Result<Vec<SavedQuery>, String> {
    let queries = state.saved_queries.read();
    let mut list: Vec<SavedQuery> = queries.values().cloned().collect();
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(list)
}

#[tauri::command]
pub async fn upsert_saved_query(state: State<'_, AppState>, query: SavedQuery) -> Result<(), String> {
    {
        let mut queries = state.saved_queries.write();
        queries.insert(query.id, query);
    }
    state.save_queries()
}

#[tauri::command]
pub async fn delete_saved_query(state: State<'_, AppState>, id: Uuid) -> Result<(), String> {
    {
        let mut queries = state.saved_queries.write();
        queries.remove(&id);
    }
    state.save_queries()
}
