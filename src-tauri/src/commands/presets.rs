//! Presets personnels : liste, création, renommage, suppression. Les presets
//! s'appliquent dans le Retoucher (voir `develop_preset_apply`).

use std::path::PathBuf;

use tauri::State;

use super::error::{CommandError, CommandResult};
use super::{AppState, blocking};
use crate::develop::presets::{self, PresetError, PresetGroup, PresetSummary, USER_FILE};
use crate::develop::settings::DevelopSettings;

impl From<PresetError> for CommandError {
    fn from(e: PresetError) -> Self {
        let code = match &e {
            PresetError::Unknown(_) => "presetMissing",
            PresetError::NameTaken(_) => "nameTaken",
            PresetError::InvalidName => "invalidName",
            _ => "internal",
        };
        Self::new(code, e)
    }
}

/// Fichier des presets personnels, à côté de la base.
pub fn user_file(state: &AppState) -> PathBuf {
    state
        .db_path
        .parent()
        .map_or_else(|| PathBuf::from(USER_FILE), |d| d.join(USER_FILE))
}

#[tauri::command]
pub async fn presets_list(state: State<'_, AppState>) -> CommandResult<Vec<PresetGroup>> {
    let path = user_file(&state);
    blocking(move || Ok(presets::list(&path)?)).await
}

/// Nouveau preset personnel depuis les réglages affichés : seuls les
/// réglages `controls` sont gardés.
#[tauri::command]
pub async fn presets_create(
    state: State<'_, AppState>,
    name: String,
    settings: DevelopSettings,
    controls: Vec<String>,
) -> CommandResult<PresetSummary> {
    let path = user_file(&state);
    blocking(move || {
        Ok(presets::create(
            &path,
            &name,
            &settings.sanitized(),
            controls,
        )?)
    })
    .await
}

#[tauri::command]
pub async fn presets_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CommandResult<()> {
    let path = user_file(&state);
    blocking(move || Ok(presets::rename(&path, &id, &name)?)).await
}

#[tauri::command]
pub async fn presets_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let path = user_file(&state);
    blocking(move || Ok(presets::delete(&path, &id)?)).await
}
