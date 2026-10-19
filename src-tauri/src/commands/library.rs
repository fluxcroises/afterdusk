use std::path::{Path, PathBuf};

use tauri::State;

use super::error::{CommandError, CommandResult};
use super::{AppState, blocking};
use crate::library::cloud::{self, CloudFolder};
use crate::library::identity::{self, Identity};
use crate::library::{self, Library, paths, template};

#[tauri::command]
pub async fn libraries_list(state: State<'_, AppState>) -> CommandResult<Vec<Library>> {
    let conn = state.connect()?;
    blocking(move || Ok(library::list_libraries(&conn)?)).await
}

#[tauri::command]
pub async fn library_create(
    state: State<'_, AppState>,
    name: String,
    root: PathBuf,
    backup: Option<PathBuf>,
) -> CommandResult<Library> {
    let conn = state.connect()?;
    blocking(move || {
        // Première ouverture : le dossier proposé (Images/Afterdusk) est créé.
        create_folder(&root)?;
        if let Some(backup) = &backup {
            create_folder(backup)?;
        }
        Ok(library::create_library(
            &conn,
            &name,
            &root,
            backup.as_deref(),
        )?)
    })
    .await
}

/// Crée un dossier choisi par l'utilisateur (bibliothèque, sauvegarde).
fn create_folder(path: &Path) -> CommandResult<()> {
    std::fs::create_dir_all(path).map_err(|e| CommandError::new("missingFolder", e))
}

/// Dossier de sauvegarde d'une bibliothèque, ou aucun.
#[tauri::command]
pub async fn library_set_backup(
    state: State<'_, AppState>,
    id: i64,
    backup: Option<PathBuf>,
) -> CommandResult<Library> {
    let conn = state.connect()?;
    blocking(move || {
        if let Some(backup) = &backup {
            create_folder(backup)?;
        }
        library::set_backup(&conn, id, backup.as_deref())?;
        Ok(library::get_library(&conn, id)?)
    })
    .await
}

#[tauri::command]
pub async fn library_set_templates(
    state: State<'_, AppState>,
    id: i64,
    folder_template: String,
    rename_template: Option<String>,
) -> CommandResult<Library> {
    let conn = state.connect()?;
    blocking(move || {
        library::set_templates(&conn, id, &folder_template, rename_template.as_deref())?;
        Ok(library::get_library(&conn, id)?)
    })
    .await
}

/// Aperçu d'un rangement et d'un renommage sur une photo d'exemple.
#[tauri::command]
pub fn template_preview(
    folder_template: String,
    rename_template: Option<String>,
    album: String,
) -> CommandResult<String> {
    template::preview(&folder_template, rename_template.as_deref(), &album)
        .map_err(|e| CommandError::new("invalidTemplate", e))
}

/// Dossiers synchronisés par un service de cloud, proposés comme sauvegarde.
#[tauri::command]
pub async fn cloud_folders() -> CommandResult<Vec<CloudFolder>> {
    blocking(|| Ok(cloud::detect(&cloud::Places::current()))).await
}

/// La sauvegarde serait-elle sur le même disque que la bibliothèque ?
#[tauri::command]
pub fn same_volume(a: PathBuf, b: PathBuf) -> Option<bool> {
    paths::same_volume(&a, &b)
}

#[tauri::command]
pub async fn identity_get(state: State<'_, AppState>) -> CommandResult<Identity> {
    let conn = state.connect()?;
    blocking(move || Ok(identity::get(&conn)?)).await
}

#[tauri::command]
pub async fn identity_set(
    state: State<'_, AppState>,
    identity: Identity,
) -> CommandResult<Identity> {
    let conn = state.connect()?;
    blocking(move || Ok(identity::set(&conn, identity)?)).await
}

/// Événement : le disque d'une bibliothèque a été débranché ou rebranché.
pub const EVENT_LIBRARY_STATUS: &str = "library-status";

/// Intervalle entre deux vérifications des disques des bibliothèques.
const LIBRARY_POLL: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub id: i64,
    pub online: bool,
}

/// Surveille les disques des bibliothèques. Au retour d'un disque : les XMP en
/// attente sont écrits et les aperçus manquants calculés, sans rien demander.
pub fn watch_libraries(app: tauri::AppHandle, db_path: PathBuf) {
    use tauri::{Emitter, Manager};
    std::thread::spawn(move || {
        let Ok(conn) = crate::db::connect(&db_path) else {
            return;
        };
        let mut known: std::collections::HashMap<i64, bool> = std::collections::HashMap::new();
        loop {
            if let Ok(libraries) = library::list_libraries(&conn) {
                for lib in libraries {
                    let before = known.insert(lib.id, lib.is_online);
                    if before == Some(lib.is_online) {
                        continue;
                    }
                    let _ = library::record_presence(&conn, lib.id, lib.is_online);
                    // Au démarrage, l'état est relevé sans être annoncé.
                    if before.is_none() {
                        continue;
                    }
                    let _ = app.emit(
                        EVENT_LIBRARY_STATUS,
                        LibraryStatus {
                            id: lib.id,
                            online: lib.is_online,
                        },
                    );
                    if lib.is_online {
                        app.state::<super::marks::XmpWriter>().wake();
                        super::previews::enqueue_missing(&app);
                    }
                }
            }
            std::thread::sleep(LIBRARY_POLL);
        }
    });
}

/// Bibliothèque hors ligne : notes en attente d'écriture et dernier passage.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineInfo {
    pub pending_xmp: i64,
    pub last_seen: Option<String>,
}

#[tauri::command]
pub async fn library_offline_info(
    state: State<'_, AppState>,
    id: i64,
) -> CommandResult<OfflineInfo> {
    let conn = state.connect()?;
    blocking(move || {
        Ok(OfflineInfo {
            pending_xmp: library::pending_xmp(&conn, id)?,
            last_seen: library::last_seen(&conn, id)?,
        })
    })
    .await
}
