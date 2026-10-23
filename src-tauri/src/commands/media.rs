use rusqlite::OptionalExtension;
use tauri::State;

use super::error::CommandResult;
use super::{AppState, blocking};
use crate::library::copies;
use crate::library::filter::{self, FilterOptions, MediaFilter, MediaSort};
use crate::library::media::{
    self, AlbumSummary, Highlight, MediaDetails, MediaQuery, MediaSummary,
};

#[tauri::command]
pub async fn media_list(
    state: State<'_, AppState>,
    library_id: i64,
    album_id: Option<i64>,
    trashed: Option<bool>,
    filter: Option<MediaFilter>,
    sort: Option<MediaSort>,
    expanded: Option<Vec<i64>>,
) -> CommandResult<Vec<MediaSummary>> {
    let conn = state.connect()?;
    let query = MediaQuery {
        album_id,
        trashed: trashed.unwrap_or(false),
        filter: filter.unwrap_or_default(),
        sort: sort.unwrap_or_default(),
        expanded: expanded.unwrap_or_default(),
    };
    let cache_root = state.cache_root.clone();
    blocking(move || {
        let mut list = media::list_media(&conn, library_id, &query)?;
        for m in list.iter_mut().filter(|m| m.copy_of.is_some()) {
            m.has_preview = copies::has_previews(&cache_root, m.id);
        }
        Ok(list)
    })
    .await
}

/// Boîtiers, objectifs et période proposés par la barre de filtres.
#[tauri::command]
pub async fn filter_options(
    state: State<'_, AppState>,
    library_id: i64,
) -> CommandResult<FilterOptions> {
    let conn = state.connect()?;
    blocking(move || Ok(filter::filter_options(&conn, library_id)?)).await
}

#[tauri::command]
pub async fn albums_list(
    state: State<'_, AppState>,
    library_id: i64,
) -> CommandResult<Vec<AlbumSummary>> {
    let conn = state.connect()?;
    blocking(move || Ok(media::list_albums(&conn, library_id)?)).await
}

#[tauri::command]
pub async fn media_details(
    state: State<'_, AppState>,
    media_id: i64,
) -> CommandResult<Option<MediaDetails>> {
    let conn = state.connect()?;
    blocking(move || Ok(media::media_details(&conn, media_id)?)).await
}

#[tauri::command]
pub async fn album_set_cover(
    state: State<'_, AppState>,
    album_id: i64,
    media_id: i64,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || {
        let media_id = copies::resolve(&conn, media_id)
            .map_err(crate::library::LibraryError::from)?
            .map_or(media_id, |(m, _)| m);
        Ok(crate::library::set_album_cover(&conn, album_id, media_id)?)
    })
    .await
}

/// Candidates pour la photo d'illustration de l'accueil (l'interface en tire une).
#[tauri::command]
pub async fn welcome_highlights(
    state: State<'_, AppState>,
    library_id: i64,
) -> CommandResult<Vec<Highlight>> {
    let conn = state.connect()?;
    blocking(move || Ok(media::highlights(&conn, library_id, 12)?)).await
}

/// Histogramme de l'aperçu d'une photo (voir `library::media::preview_histogram`).
#[tauri::command]
pub async fn media_histogram(
    state: State<'_, AppState>,
    media_id: i64,
) -> CommandResult<Option<Vec<u32>>> {
    let cache = state.cache_root.clone();
    blocking(move || Ok(crate::library::media::preview_histogram(&cache, media_id))).await
}

/// Montre le fichier d'origine dans l'Explorateur ou le Finder, sélectionné.
/// Rien n'est ouvert ni modifié ; une photo d'un disque débranché est ignorée.
#[tauri::command]
pub async fn media_reveal(state: State<'_, AppState>, media_id: i64) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || {
        let found: Option<(i64, String)> = conn
            .query_row(
                "SELECT library_id, path FROM media WHERE id = ?1",
                [media_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(crate::library::LibraryError::from)?;
        let Some((library_id, rel)) = found else {
            return Ok(());
        };
        let lib = crate::library::get_library(&conn, library_id)?;
        if !lib.is_online {
            return Ok(());
        }
        let Ok(path) = crate::library::paths::to_absolute(&lib.root_path, &rel) else {
            return Ok(());
        };
        if path.is_file() {
            reveal(&path);
        }
        Ok(())
    })
    .await
}

fn reveal(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // « /select, » et le chemin entre guillemets, tels quels : Explorer ne
        // comprend pas le guillemetage habituel des arguments.
        let _ = std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    if let Some(dir) = path.parent() {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}
