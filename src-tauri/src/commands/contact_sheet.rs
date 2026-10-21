//! Planches contact en PDF, écrites en arrière-plan.

use std::path::PathBuf;

use image::DynamicImage;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::error::{CommandError, CommandResult};
use super::{AppState, blocking};
use crate::analysis::run::source_image;
use crate::export::contact_sheet::{self, ContactSheet};
use crate::library::identity;
use crate::library::media::cached_preview;

pub const EVENT_PROGRESS: &str = "contact-sheet-progress";

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

#[tauri::command]
pub async fn contact_sheet_export(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
    sheet: ContactSheet,
) -> CommandResult<()> {
    let conn = state.connect()?;
    let dir = state.data_dir();
    let cache = state.cache_root.clone();
    blocking(move || {
        let logo = identity::logo(&conn, &dir);
        // Aperçu en cache (retouches comprises), sinon l'original.
        let thumb = |id: i64| -> Option<DynamicImage> {
            [1024, 256]
                .into_iter()
                .find_map(|size| cached_preview(&cache, id, size))
                .and_then(|p| image::open(p).ok())
                .or_else(|| source_image(&conn, &cache, id, 1024).map(DynamicImage::ImageRgb8))
        };
        let bytes = contact_sheet::render(&sheet, logo.as_ref(), thumb, |done, total| {
            let _ = app.emit(EVENT_PROGRESS, Progress { done, total });
        })
        .ok_or_else(|| CommandError::new("contactSheet", "police illisible"))?;
        crate::pdf::write_atomic(&path, &bytes)
            .map_err(|e| CommandError::new("contactSheet", e))?;
        Ok(())
    })
    .await
}
