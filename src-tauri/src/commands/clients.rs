//! Clients et types de séance (décision 0009).

use tauri::State;

use super::error::CommandResult;
use super::{AppState, blocking};
use crate::library::clients::{self, Client, ClientAlbum, ClientInput, SessionType};

#[tauri::command]
pub async fn clients_list(state: State<'_, AppState>) -> CommandResult<Vec<Client>> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::list(&conn)?)).await
}

#[tauri::command]
pub async fn client_create(state: State<'_, AppState>, client: ClientInput) -> CommandResult<i64> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::create(&conn, &client)?)).await
}

#[tauri::command]
pub async fn client_update(
    state: State<'_, AppState>,
    id: i64,
    client: ClientInput,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::update(&conn, id, &client)?)).await
}

/// Efface la fiche ; les albums restent, sans client.
#[tauri::command]
pub async fn client_delete(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::delete(&conn, id)?)).await
}

#[tauri::command]
pub async fn client_albums(state: State<'_, AppState>, id: i64) -> CommandResult<Vec<ClientAlbum>> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::albums(&conn, id)?)).await
}

/// Rattache (ou détache, avec `null`) un client et un type de séance à un album.
#[tauri::command]
pub async fn album_set_client(
    state: State<'_, AppState>,
    album_id: i64,
    client_id: Option<i64>,
    session_type_id: Option<i64>,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || {
        Ok(clients::set_album(
            &conn,
            album_id,
            client_id,
            session_type_id,
        )?)
    })
    .await
}

#[tauri::command]
pub async fn session_types_list(state: State<'_, AppState>) -> CommandResult<Vec<SessionType>> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::session_types(&conn)?)).await
}

#[tauri::command]
pub async fn session_type_create(state: State<'_, AppState>, name: String) -> CommandResult<i64> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::create_session_type(&conn, &name)?)).await
}

#[tauri::command]
pub async fn session_type_rename(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::rename_session_type(&conn, id, &name)?)).await
}

#[tauri::command]
pub async fn session_type_delete(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(clients::delete_session_type(&conn, id)?)).await
}
