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
