use tauri::State;

use super::error::CommandResult;
use super::{AppState, blocking};
use crate::library::media::{self, AlbumSummary, MediaDetails, MediaSummary};

#[tauri::command]
pub async fn media_list(
    state: State<'_, AppState>,
    library_id: i64,
    album_id: Option<i64>,
) -> CommandResult<Vec<MediaSummary>> {
    let conn = state.connect()?;
    blocking(move || Ok(media::list_media(&conn, library_id, album_id)?)).await
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
