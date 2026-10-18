pub mod commands;
pub mod db;
pub mod import;
pub mod library;
pub mod metadata;
pub mod raw;

use tauri::Manager;

use commands::AppState;

const DATABASE_FILE: &str = "afterdusk.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Base dans le dossier de données de l'application (cahier des charges, section 3).
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join(DATABASE_FILE);
            let conn = db::open(&db_path)?;
            // Un import « en cours » au démarrage a été coupé : il sera proposé à la reprise.
            import::mark_interrupted(&conn)?;
            app.manage(AppState::new(db_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::library::libraries_list,
            commands::library::library_create,
            commands::import::source_scan,
            commands::import::import_start,
            commands::import::import_resume,
            commands::import::import_cancel,
            commands::import::imports_resumable,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Afterdusk n'a pas pu démarrer : {error}");
        std::process::exit(1);
    }
}
