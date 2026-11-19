mod commands;
mod db;
mod error;
mod library;
mod settings;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db = app
                .path()
                .app_data_dir()
                .map_err(|e| {
                    error::AppError::new(
                        "db_open_failed",
                        format!("Pasta de dados do aplicativo indisponível: {e}"),
                    )
                })
                .and_then(|dir| {
                    std::fs::create_dir_all(&dir)?;
                    db::open(&dir.join("masterofnotes.db"))
                });
            if let Err(e) = &db {
                log::error!("database unavailable: {e}");
            }
            app.manage(commands::AppState::new(db));
            commands::restore_library(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::set_library_folder,
            commands::rescan_library,
            commands::list_songs,
            commands::update_song,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
