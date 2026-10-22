//! Galeries web : génération en arrière-plan, puis relecture de la
//! sélection renvoyée par le client.

use image::DynamicImage;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::error::{CommandError, CommandResult};
use super::marks::XmpWriter;
use super::{AppState, blocking};
use crate::analysis::run::source_image;
use crate::export::gallery::{self, GalleryRequest, GalleryResult, SelectionMatch};
use crate::library::marks::{self, Mark};
use crate::library::media::cached_preview;
use crate::library::{LibraryError, collections, copies, identity};
use crate::metadata::xmp::Flag;

pub const EVENT_PROGRESS: &str = "gallery-progress";

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

#[tauri::command]
pub async fn gallery_create(
    app: AppHandle,
    state: State<'_, AppState>,
    request: GalleryRequest,
) -> CommandResult<GalleryResult> {
    let conn = state.connect()?;
    let cache = state.cache_root.clone();
    blocking(move || {
        let email = identity::get(&conn).map(|i| i.email).unwrap_or_default();
        // Aperçu en cache (retouches comprises), sinon l'original.
        let image_of = |id: i64| -> Option<DynamicImage> {
            [2048, 1024]
                .into_iter()
                .find_map(|size| cached_preview(&cache, id, size))
                .and_then(|p| image::open(p).ok())
                .or_else(|| {
                    source_image(&conn, &cache, id, gallery::PHOTO_SIDE)
                        .map(DynamicImage::ImageRgb8)
                })
        };
        gallery::generate(&conn, &request, &email, image_of, |done, total| {
            let _ = app.emit(EVENT_PROGRESS, Progress { done, total });
        })
        .map_err(|e| CommandError::new("gallery", e))
    })
    .await
}

/// Galerie et photos d'une sélection collée ou ouverte ; erreur si le texte
/// ne contient pas de sélection, ou si la galerie est inconnue.
#[tauri::command]
pub async fn gallery_read_selection(
    state: State<'_, AppState>,
    text: String,
) -> CommandResult<SelectionMatch> {
    let conn = state.connect()?;
    blocking(move || {
        let parsed = gallery::parse_selection(&text)
            .ok_or_else(|| CommandError::new("selectionUnreadable", ""))?;
        gallery::match_selection(&conn, &parsed)
            .map_err(LibraryError::from)?
            .ok_or_else(|| CommandError::new("galleryUnknown", parsed.code))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionApplied {
    pub flagged: usize,
    pub collection_id: i64,
}

/// Pose « Retenue » sur les photos choisies et les réunit dans une
/// collection manuelle `collection_name`.
#[tauri::command]
pub async fn gallery_apply_selection(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    text: String,
    collection_name: String,
) -> CommandResult<SelectionApplied> {
    let mut conn = state.connect()?;
    let applied = blocking(move || {
        let parsed = gallery::parse_selection(&text)
            .ok_or_else(|| CommandError::new("selectionUnreadable", ""))?;
        let found = gallery::match_selection(&conn, &parsed)
            .map_err(LibraryError::from)?
            .ok_or_else(|| CommandError::new("galleryUnknown", parsed.code))?;
        // Copies virtuelles : les marques sont celles de leur photo.
        let ids = copies::media_ids(&conn, &found.media_ids).map_err(LibraryError::from)?;
        let flagged = marks::apply_mark(&mut conn, &ids, Mark::Flag(Flag::Picked))?;
        let collection_id = collections::create(&conn, found.library_id, &collection_name, None)?;
        collections::add_media(&mut conn, collection_id, &ids)?;
        Ok(SelectionApplied {
            flagged,
            collection_id,
        })
    })
    .await?;
    writer.wake();
    Ok(applied)
}

/// Texte d'un fichier de sélection (`selection-AD-….txt`), 1 Mo au plus.
#[tauri::command]
pub async fn gallery_selection_file(path: std::path::PathBuf) -> CommandResult<String> {
    blocking(move || {
        let size = std::fs::metadata(&path)
            .map_err(|e| CommandError::new("selectionUnreadable", e))?
            .len();
        if size > 1_000_000 {
            return Err(CommandError::new(
                "selectionUnreadable",
                "fichier trop grand",
            ));
        }
        let bytes =
            std::fs::read(&path).map_err(|e| CommandError::new("selectionUnreadable", e))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
    .await
}
