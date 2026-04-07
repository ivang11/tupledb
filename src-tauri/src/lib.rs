pub mod commands;
pub mod connections;
pub mod driver;
pub mod ssh;
pub mod mysql;
pub mod schema;
pub mod filters;
pub mod query_builder;
pub mod security;
pub mod state;

use tauri::Manager;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState::new(app.handle()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_connections,
            crate::commands::add_connection,
            crate::commands::remove_connection,
            crate::commands::test_connection,
            crate::commands::connect,
            crate::schema::get_databases,
            crate::schema::create_database,
            crate::schema::drop_database,
            crate::schema::get_tables,
            crate::schema::get_table_structure,
            crate::schema::export_database,
            crate::schema::import_sql,
            crate::mysql::get_table_data,
            crate::mysql::export_table,
            crate::mysql::apply_table_changes,
            crate::mysql::insert_row,
            crate::mysql::drop_table,
            crate::mysql::truncate_table,
            crate::schema::get_foreign_keys,
            crate::schema::get_table_indexes,
            crate::schema::get_table_ddl,
            crate::mysql::execute_query,
            crate::mysql::cancel_query,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
