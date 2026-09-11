pub mod benchmark;
pub mod commands;
pub mod connection_store;
pub mod connections;
pub mod database;
pub mod filters;
pub mod saved_queries;
pub mod security;
pub mod services;
pub mod ssh;
pub mod state;

use crate::state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    crate::benchmark::mark_process_started();
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
            crate::benchmark::benchmark_config,
            crate::benchmark::report_benchmark_metrics,
            crate::commands::connections::get_connections,
            crate::commands::connections::get_connection_storage_info,
            crate::commands::connections::add_connection,
            crate::commands::connections::remove_connection,
            crate::commands::connections::test_connection,
            crate::commands::connections::connect,
            crate::commands::connections::disconnect,
            crate::commands::connections::get_available_drivers,
            crate::commands::connections::export_connections,
            crate::commands::connections::import_connections,
            crate::commands::catalog::get_databases,
            crate::commands::catalog::get_database_creation_options,
            crate::commands::catalog::create_database,
            crate::commands::catalog::drop_database,
            crate::commands::catalog::get_tables,
            crate::commands::catalog::get_table_structure,
            crate::commands::transfers::export_database,
            crate::commands::transfers::cancel_export,
            crate::commands::transfers::import_sql,
            crate::commands::transfers::cancel_import,
            crate::commands::tables::get_table_data,
            crate::commands::transfers::export_table,
            crate::commands::tables::apply_table_changes,
            crate::commands::tables::insert_row,
            crate::commands::tables::alter_table_column,
            crate::commands::tables::drop_table,
            crate::commands::tables::drop_tables,
            crate::commands::tables::truncate_table,
            crate::commands::catalog::get_foreign_keys,
            crate::commands::catalog::get_table_indexes,
            crate::commands::catalog::get_table_ddl,
            crate::commands::queries::execute_query,
            crate::commands::queries::cancel_query,
            crate::saved_queries::get_saved_queries,
            crate::saved_queries::upsert_saved_query,
            crate::saved_queries::delete_saved_query,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
