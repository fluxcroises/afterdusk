use std::path::PathBuf;

use tauri::State;

use super::error::CommandResult;
use super::{AppState, blocking};
use crate::library::{self, Library};

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
        Ok(library::create_library(
            &conn,
            &name,
            &root,
            backup.as_deref(),
        )?)
    })
    .await
}
