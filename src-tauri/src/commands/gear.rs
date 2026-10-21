//! Matériel : inventaire, fiches, fusion, masquage, accessoires, photo.

use std::path::PathBuf;

use tauri::State;
use tauri::ipc::Response;

use super::error::{CommandError, CommandResult};
use super::{AppState, blocking};
use crate::gear::maintenance::{self, MaintenanceEntry, MaintenanceInput, Reminder};
use crate::gear::stats::GearStats;
use crate::gear::{self, GearError, GearItem, GearKind, GearRecord, GearUpdate};

impl From<GearError> for CommandError {
    fn from(e: GearError) -> Self {
        let code = match e {
            GearError::NotFound(_) => "gearNotFound",
            GearError::KindMismatch => "gearKindMismatch",
            GearError::Detected => "gearDetected",
            GearError::Photo(_) => "gearPhoto",
            GearError::MaintenanceNotFound(_) => "maintenanceNotFound",
            GearError::InvalidDate(_) => "invalidDate",
            GearError::Sqlite(_) => "internal",
        };
        Self::new(code, e)
    }
}

/// Dossier des données de l'application (celui de la base).
fn data_dir(state: &AppState) -> PathBuf {
    state
        .db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
}

#[tauri::command]
pub async fn gear_list(
    state: State<'_, AppState>,
    include_hidden: bool,
) -> CommandResult<Vec<GearItem>> {
    let conn = state.connect()?;
    blocking(move || Ok(gear::list(&conn, include_hidden)?)).await
}

#[tauri::command]
pub async fn gear_get(state: State<'_, AppState>, id: i64) -> CommandResult<GearRecord> {
    let conn = state.connect()?;
    blocking(move || Ok(gear::get(&conn, id)?)).await
}

#[tauri::command]
pub async fn gear_stats(state: State<'_, AppState>, id: i64) -> CommandResult<GearStats> {
    let conn = state.connect()?;
    blocking(move || Ok(gear::stats::stats(&conn, id)?)).await
}

#[tauri::command]
pub async fn gear_update(
    state: State<'_, AppState>,
    id: i64,
    update: GearUpdate,
) -> CommandResult<GearRecord> {
    let conn = state.connect()?;
    blocking(move || Ok(gear::update(&conn, id, update)?)).await
}

#[tauri::command]
pub async fn gear_create(
    state: State<'_, AppState>,
    kind: GearKind,
    make: String,
    model: String,
) -> CommandResult<i64> {
    let conn = state.connect()?;
    blocking(move || Ok(gear::create(&conn, kind, &make, &model)?)).await
}

#[tauri::command]
pub async fn gear_delete(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    let conn = state.connect()?;
    let dir = data_dir(&state);
    blocking(move || Ok(gear::delete(&conn, &dir, id)?)).await
}

#[tauri::command]
pub async fn gear_set_hidden(
    state: State<'_, AppState>,
    id: i64,
    hidden: bool,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(gear::set_hidden(&conn, id, hidden)?)).await
}

#[tauri::command]
pub async fn gear_set_confirmed(
    state: State<'_, AppState>,
    id: i64,
    confirmed: bool,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(gear::set_confirmed(&conn, id, confirmed)?)).await
}

#[tauri::command]
pub async fn gear_merge(
    state: State<'_, AppState>,
    source: i64,
    target: i64,
) -> CommandResult<GearRecord> {
    let mut conn = state.connect()?;
    blocking(move || Ok(gear::merge(&mut conn, source, target)?)).await
}

#[tauri::command]
pub async fn gear_set_photo_media(
    state: State<'_, AppState>,
    id: i64,
    media_id: Option<i64>,
) -> CommandResult<()> {
    let conn = state.connect()?;
    let dir = data_dir(&state);
    blocking(move || Ok(gear::set_photo_media(&conn, &dir, id, media_id)?)).await
}

#[tauri::command]
pub async fn gear_set_photo_file(
    state: State<'_, AppState>,
    id: i64,
    path: PathBuf,
) -> CommandResult<()> {
    let conn = state.connect()?;
    let dir = data_dir(&state);
    blocking(move || Ok(gear::set_photo_file(&conn, &dir, id, &path)?)).await
}

/// Image déposée sur la fiche, en octets (vide s'il n'y en a pas).
#[tauri::command]
pub async fn gear_photo_file(state: State<'_, AppState>, id: i64) -> CommandResult<Response> {
    let conn = state.connect()?;
    let dir = data_dir(&state);
    let bytes = blocking(move || Ok(gear::photo_file(&conn, &dir, id)?)).await?;
    Ok(Response::new(bytes.unwrap_or_default()))
}

#[tauri::command]
pub async fn maintenance_list(
    state: State<'_, AppState>,
    gear_id: i64,
) -> CommandResult<Vec<MaintenanceEntry>> {
    let conn = state.connect()?;
    blocking(move || Ok(maintenance::list(&conn, gear_id)?)).await
}

#[tauri::command]
pub async fn maintenance_add(
    state: State<'_, AppState>,
    gear_id: i64,
    input: MaintenanceInput,
) -> CommandResult<i64> {
    let conn = state.connect()?;
    blocking(move || Ok(maintenance::add(&conn, gear_id, &input)?)).await
}

#[tauri::command]
pub async fn maintenance_update(
    state: State<'_, AppState>,
    id: i64,
    input: MaintenanceInput,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(maintenance::update(&conn, id, &input)?)).await
}

#[tauri::command]
pub async fn maintenance_delete(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(maintenance::delete(&conn, id)?)).await
}

/// Rappels d'un appareil, ou de tout le matériel visible, à la date du jour.
#[tauri::command]
pub async fn maintenance_reminders(
    state: State<'_, AppState>,
    gear_id: Option<i64>,
) -> CommandResult<Vec<Reminder>> {
    let conn = state.connect()?;
    let today = chrono::Local::now().date_naive();
    blocking(move || Ok(maintenance::reminders(&conn, gear_id, today)?)).await
}
