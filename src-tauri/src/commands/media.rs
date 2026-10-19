use tauri::State;

use super::error::CommandResult;
use super::{AppState, blocking};
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
    blocking(move || Ok(media::list_media(&conn, library_id, &query)?)).await
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
    blocking(move || Ok(crate::library::set_album_cover(&conn, album_id, media_id)?)).await
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
