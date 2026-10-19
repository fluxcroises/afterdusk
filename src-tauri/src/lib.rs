pub mod commands;
pub mod db;
pub mod develop;
pub mod devices;
pub mod import;
pub mod library;
pub mod metadata;
pub mod raw;

use tauri::Manager;

use commands::AppState;
use commands::marks::XmpWriter;
use commands::previews::{self, PreviewQueue};

const DATABASE_FILE: &str = "afterdusk.db";
const PREVIEW_DIR: &str = "previews";

pub const MAIN_WINDOW: &str = "main";
pub const WELCOME_WINDOW: &str = "welcome";

/// Fermer la fenêtre principale quitte l'application. Fermer l'accueil le
/// masque seulement (il peut être rouvert), sauf s'il était seul à l'écran.
fn on_close_requested(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let app = window.app_handle();
    let main_visible = app
        .get_webview_window(MAIN_WINDOW)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    match window.label() {
        MAIN_WINDOW => app.exit(0),
        WELCOME_WINDOW if main_visible => {
            api.prevent_close();
            let _ = window.hide();
        }
        // Le second écran se ferme sans fermer l'application.
        commands::windows::SECONDARY_WINDOW => {}
        _ => app.exit(0),
    }
}

/// Vidage automatique de la corbeille, en arrière-plan pour ne pas retarder
/// l'ouverture : photos jetées depuis plus longtemps que le délai réglé.
fn purge_expired_trash(db_path: std::path::PathBuf, cache_root: std::path::PathBuf) {
    std::thread::spawn(move || {
        let result = db::connect(&db_path)
            .map_err(|e| e.to_string())
            .and_then(|mut conn| {
                let days = library::trash::retention_days(&conn).map_err(|e| e.to_string())?;
                library::trash::purge_expired(&mut conn, &cache_root, days)
                    .map_err(|e| e.to_string())
            });
        match result {
            Ok(r) if r.done + r.failed > 0 => {
                eprintln!("corbeille : {} supprimées, {} en échec", r.done, r.failed)
            }
            Ok(_) => {}
            Err(err) => eprintln!("vidage de la corbeille : {err}"),
        }
    });
}

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
            purge_expired_trash(db_path.clone(), cache_root.clone());
            app.manage(XmpWriter::start(db_path.clone()));
            app.manage(AppState::new(db_path, cache_root));
            app.manage(queue);
            commands::library::watch_libraries(
                app.handle().clone(),
                app.state::<AppState>().db_path.clone(),
            );
            app.manage(commands::devices::DeviceWatch::start(app.handle().clone()));
            previews::enqueue_missing(app.handle());
            // Essai de lancement (CI) : échec si l'interface ne s'affiche pas à temps.
            if std::env::var_os(commands::SMOKE_ENV).is_some() {
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(90));
                    eprintln!("afterdusk : l'interface ne s'est pas affichée en 90 s");
                    std::process::exit(2);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                on_close_requested(window, api);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::app_ready,
            commands::library::libraries_list,
            commands::library::library_create,
            commands::library::library_set_backup,
            commands::library::library_set_templates,
            commands::library::template_preview,
            commands::library::same_volume,
            commands::library::cloud_folders,
            commands::library::library_offline_info,
            commands::library::identity_get,
            commands::library::identity_set,
            commands::import::source_scan,
            commands::import::import_start,
            commands::import::import_resume,
            commands::import::import_cancel,
            commands::import::imports_resumable,
            commands::import::imports_recent,
            commands::media::media_list,
            commands::media::filter_options,
            commands::media::albums_list,
            commands::media::media_details,
            commands::media::album_set_cover,
            commands::media::welcome_highlights,
            commands::trash::media_trash,
            commands::devices::devices_list,
            commands::devices::insert_behavior_get,
            commands::devices::insert_behavior_set,
            commands::devices::request_attention,
            commands::trash::media_restore,
            commands::trash::media_trash_format,
            commands::trash::pile_set_top,
            commands::trash::trash_empty,
            commands::trash::trash_summary,
            commands::trash::trash_retention_get,
            commands::trash::trash_retention_set,
            commands::windows::secondary_toggle,
            commands::marks::media_set_rating,
            commands::marks::media_set_flag,
            commands::marks::media_set_color,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Afterdusk n'a pas pu démarrer : {error}");
        std::process::exit(1);
    }
}
