pub mod commands;
pub mod connections;
pub mod driver;
pub mod filters;
pub mod mysql;
pub mod query_builder;
pub mod saved_queries;
pub mod schema;
pub mod security;
pub mod ssh;
pub mod state;

use crate::state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.manage(AppState::new(app.handle()));

            if let (Some(window), Some(icon)) =
                (app.get_webview_window("main"), app.default_window_icon())
            {
                window.set_icon(icon.clone())?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_connections,
            crate::commands::add_connection,
            crate::commands::remove_connection,
            crate::commands::test_connection,
            crate::commands::connect,
            crate::commands::export_connections,
            crate::commands::import_connections,
            crate::schema::get_databases,
            crate::schema::create_database,
            crate::schema::drop_database,
            crate::schema::get_tables,
            crate::schema::get_table_structure,
            crate::schema::export_database,
            crate::schema::cancel_export,
            crate::schema::import_sql,
            crate::schema::cancel_import,
            crate::mysql::get_table_data,
            crate::mysql::export_table,
            crate::mysql::apply_table_changes,
            crate::mysql::insert_row,
            crate::mysql::drop_table,
            crate::mysql::drop_tables,
            crate::mysql::truncate_table,
            crate::schema::get_foreign_keys,
            crate::schema::get_table_indexes,
            crate::schema::get_table_ddl,
            crate::mysql::execute_query,
            crate::mysql::cancel_query,
            crate::saved_queries::get_saved_queries,
            crate::saved_queries::upsert_saved_query,
            crate::saved_queries::delete_saved_query,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
