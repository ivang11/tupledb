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

const REPOSITORY_URL: &str = "https://github.com/ivang11/tupledb";

/// Native menu bar for macOS. Picks are forwarded to
/// the main window as `tupledb:menu` events; items that mirror in-app
/// shortcuts carry no accelerator so the customizable keybindings keep working.
#[cfg(target_os = "macos")]
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{AboutMetadataBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};

    let about = AboutMetadataBuilder::new()
        .name(Some("TupleDB"))
        .version(Some(app.package_info().version.to_string()))
        // The window icon is tiny; the About panel wants the full-size artwork.
        .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png")).ok())
        .website(Some(REPOSITORY_URL))
        .website_label(Some("GitHub"))
        .build();

    let app_menu = SubmenuBuilder::new(app, "TupleDB")
        .item(&PredefinedMenuItem::about(app, Some("About TupleDB"), Some(about))?)
        .separator()
        .item(&MenuItemBuilder::with_id("settings", "Settings…").accelerator("CmdOrCtrl+,").build(app)?)
        .item(&MenuItemBuilder::with_id("check-updates", "Check for Updates…").build(app)?)
        .separator()
        .services()
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some("Hide TupleDB"))?)
        .hide_others()
        .show_all()
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("Quit TupleDB"))?)
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&MenuItemBuilder::with_id("new-connection", "New Connection…").accelerator("CmdOrCtrl+N").build(app)?)
        .item(&MenuItemBuilder::with_id("new-query", "New Query").accelerator("CmdOrCtrl+T").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("import-connections", "Import Connections…").build(app)?)
        .item(&MenuItemBuilder::with_id("export-connections", "Export Connections…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("close-window", "Close Window").accelerator("Shift+CmdOrCtrl+W").build(app)?)
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&MenuItemBuilder::with_id("toggle-sidebar", "Toggle Sidebar").build(app)?)
        .item(&MenuItemBuilder::with_id("quick-search", "Quick Search…").build(app)?)
        .separator()
        .fullscreen()
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .build()?;

    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&MenuItemBuilder::with_id("settings-keyboard", "Keyboard Shortcuts…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("github", "TupleDB on GitHub").build(app)?)
        .build()?;

    MenuBuilder::new(app)
        .items(&[&app_menu, &file_menu, &edit_menu, &view_menu, &window_menu, &help_menu])
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    crate::benchmark::mark_process_started();
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(AppState::new(app.handle()));

            if let (Some(window), Some(icon)) =
                (app.get_webview_window("main"), app.default_window_icon())
            {
                window.set_icon(icon.clone())?;
            }

            #[cfg(target_os = "macos")]
            {
                use tauri::Emitter;
                app.set_menu(build_menu(app.handle())?)?;
                app.on_menu_event(|app, event| {
                    let id = event.id().as_ref();
                    if id == "github" {
                        use tauri_plugin_opener::OpenerExt;
                        let _ = app.opener().open_url(REPOSITORY_URL, None::<&str>);
                        return;
                    }
                    if id == "close-window" {
                        for window in app.webview_windows().into_values() {
                            if window.is_focused().unwrap_or(false) {
                                let _ = window.close();
                            }
                        }
                        return;
                    }
                    if let Some(main) = app.get_webview_window("main") {
                        let _ = main.set_focus();
                    }
                    let _ = app.emit_to("main", "tupledb:menu", id.to_string());
                });
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
            crate::commands::queries::execute_query_batch,
            crate::commands::queries::keep_connection_alive,
            crate::commands::queries::cancel_query,
            crate::saved_queries::get_saved_queries,
            crate::saved_queries::upsert_saved_query,
            crate::saved_queries::delete_saved_query,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
