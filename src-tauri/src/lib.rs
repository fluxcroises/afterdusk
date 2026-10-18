pub mod commands;
pub mod db;
pub mod import;
pub mod library;
pub mod metadata;
pub mod raw;

use tauri::Manager;

use commands::AppState;
use commands::previews::{self, PreviewQueue};

const DATABASE_FILE: &str = "afterdusk.db";
const PREVIEW_DIR: &str = "previews";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol(previews::SCHEME, previews::protocol)
        .setup(|app| {
            // Base dans le dossier de données de l'application (cahier des charges, section 3).
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join(DATABASE_FILE);
            let conn = db::open(&db_path)?;
            // Un import « en cours » au démarrage a été coupé : il sera proposé à la reprise.
            import::mark_interrupted(&conn)?;
            // Le cache vit avec la base pour rester consultable disque débranché.
            let cache_root = dir.join(PREVIEW_DIR);
            std::fs::create_dir_all(&cache_root)?;
            let workers = std::thread::available_parallelism()
                .map(|n| (n.get() / 2).clamp(1, 4))
                .unwrap_or(1);
            let queue = PreviewQueue::start(
                app.handle().clone(),
                db_path.clone(),
                cache_root.clone(),
                workers,
            );
            app.manage(AppState::new(db_path, cache_root));
            app.manage(queue);
            previews::enqueue_missing(app.handle());
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
            commands::media::media_list,
            commands::media::albums_list,
            commands::media::media_details,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Afterdusk n'a pas pu démarrer : {error}");
        std::process::exit(1);
    }
}
