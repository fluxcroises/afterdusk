pub mod commands;
pub mod db;
pub mod develop;
pub mod devices;
pub mod export;
pub mod gear;
pub mod geo;
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

/// Nomme le lieu des photos localisées qui n'en ont pas encore (bases
/// antérieures à la V0.3), en arrière-plan et par lots.
fn name_places(db_path: std::path::PathBuf) {
    std::thread::spawn(move || {
        let Ok(conn) = db::connect(&db_path) else {
            return;
        };
        loop {
            match geo::naming::name_pending(&conn, 500) {
                Ok(0) => return,
                Ok(_) => {}
                Err(err) => {
                    eprintln!("lieux des photos : {err}");
                    return;
                }
            }
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
            name_places(db_path.clone());
            app.manage(XmpWriter::start(app.handle().clone(), db_path.clone()));
            app.manage(commands::develop::DevelopEngine::new());
            app.manage(commands::export::ExportState::default());
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
            commands::bursts::bursts_detect,
            commands::bursts::bursts_stack,
            commands::bursts::bursts_unstack,
            commands::collections::collections_list,
            commands::collections::collection_create,
            commands::collections::collection_rename,
            commands::collections::collection_set_filter,
            commands::collections::collection_delete,
            commands::collections::collection_add_media,
            commands::collections::collection_remove_media,
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
            commands::develop::develop_open,
            commands::develop::develop_render,
            commands::develop::develop_close,
            commands::develop::develop_pick_white,
            commands::develop::develop_zoom_open,
            commands::develop::develop_zoom_render,
            commands::develop::develop_geometry_constrain,
            commands::develop::develop_auto_straighten,
            commands::develop::develop_spot_source,
            commands::develop::develop_preset_apply,
            commands::develop::develop_preset_previews,
            commands::develop::develop_paste_apply,
            commands::develop::develop_paste,
            commands::export::export_settings,
            commands::export::export_settings_save,
            commands::export::export_start,
            commands::export::export_cancel,
            commands::export::export_reveal,
            commands::export::export_preview,
            commands::export::export_heif_available,
            commands::geo::geo_clock,
            commands::geo::geo_locate_gpx,
            commands::geo::geo_locate_phone,
            commands::geo::geo_search_places,
            commands::geo::geo_locate_manual,
            commands::geo::geo_time_proposals,
            commands::geo::geo_time_correct,
            commands::geo::geo_unlocated,
            commands::geo::geo_map,
            commands::geo::geo_album_tracks,
            commands::gear::gear_list,
            commands::gear::gear_get,
            commands::gear::gear_stats,
            commands::gear::maintenance_list,
            commands::gear::maintenance_add,
            commands::gear::maintenance_update,
            commands::gear::maintenance_delete,
            commands::gear::maintenance_reminders,
            commands::gear::gear_export_csv,
            commands::gear::gear_export_pdf,
            commands::gear::gear_update,
            commands::gear::gear_create,
            commands::gear::gear_delete,
            commands::gear::gear_set_hidden,
            commands::gear::gear_set_confirmed,
            commands::gear::gear_merge,
            commands::gear::gear_set_photo_media,
            commands::gear::gear_set_photo_file,
            commands::gear::gear_photo_file,
            commands::library::library_rescan,
            commands::presets::presets_list,
            commands::presets::presets_create,
            commands::presets::presets_rename,
            commands::presets::presets_delete,
            commands::develop::develop_auto_light,
            commands::develop::develop_auto_white,
            commands::develop::develop_save,
            commands::develop::develop_history,
            commands::develop::develop_go_to,
            commands::develop::develop_state,
            commands::develop::develop_version_create,
            commands::develop::develop_version_rename,
            commands::develop::develop_version_delete,
            commands::develop::develop_snapshot_create,
            commands::develop::develop_snapshot_apply,
            commands::develop::develop_snapshot_rename,
            commands::develop::develop_snapshot_delete,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Afterdusk n'a pas pu démarrer : {error}");
        std::process::exit(1);
    }
}
