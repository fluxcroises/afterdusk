//! Image de travail d'une photo de la bibliothèque : l'original est lu par
//! LibRaw, jamais écrit.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension};

use super::pipeline::WorkingImage;
use crate::library::paths::{self, PathError};
use crate::library::{LibraryError, get_library};
use crate::metadata::media::{MediaFormat, format_of};
use crate::raw::{RawError, RawFile};

/// Côté long de l'image de travail affichée à l'écran (décision 0005).
pub const PREVIEW_SIDE: u32 = 2048;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("photo {0} introuvable")]
    NotFound(i64),
    #[error("bibliothèque hors ligne : {}", .0.display())]
    Offline(PathBuf),
    #[error("retouche réservée aux RAW pour l'instant : {0}")]
    Unsupported(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Raw(#[from] RawError),
}

/// Décode l'original d'une photo (demi-taille, suffisante pour l'écran) et le
/// ramène à `max_side` pixels de côté long.
pub fn working_image(
    conn: &Connection,
    media_id: i64,
    max_side: u32,
) -> Result<WorkingImage, SourceError> {
    let (library_id, path): (i64, String) = conn
        .query_row(
            "SELECT library_id, path FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(SourceError::NotFound(media_id))?;
    let lib = get_library(conn, library_id)?;
    if !lib.is_online {
        return Err(SourceError::Offline(lib.root_path));
    }
    let abs = paths::to_absolute(&lib.root_path, &path)?;
    if format_of(&abs) != Some(MediaFormat::Raw) {
        return Err(SourceError::Unsupported(path));
    }
    let mut raw = RawFile::open(&abs)?;
    let developed = raw.develop(true)?;
    Ok(WorkingImage::from_raw(&developed, max_side))
}
