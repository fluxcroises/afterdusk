//! Image de travail d'une photo de la bibliothèque : l'original est lu par
//! LibRaw, jamais écrit.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension};

use super::pipeline::WorkingImage;
use crate::library::paths::{self, PathError};
use crate::library::{LibraryError, get_library};
use crate::metadata::media::{MediaFormat, format_of};
use crate::raw::{DevelopedRaw, RawError, RawFile};

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

/// Original RAW d'une photo de la bibliothèque, ouvert en lecture seule.
fn open_raw(conn: &Connection, media_id: i64) -> Result<RawFile, SourceError> {
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
    Ok(RawFile::open(&abs)?)
}

/// Décode l'original d'une photo et le ramène à `max_side` pixels de côté
/// long : en demi-définition (quatre fois plus rapide) quand elle suffit, en
/// pleine définition pour les capteurs trop petits.
pub fn working_image(
    conn: &Connection,
    media_id: i64,
    max_side: u32,
) -> Result<WorkingImage, SourceError> {
    let mut raw = open_raw(conn, media_id)?;
    let info = raw.info();
    let half = u32::from(info.width.max(info.height)) / 2 >= max_side;
    let developed = raw.develop(half)?;
    Ok(WorkingImage::from_raw(&developed, max_side))
}

/// Original développé en pleine définition, pour le zoom 100 %. Le X-Trans
/// y est dématricé en une passe (5 s au lieu de 11 s pour 26 Mpx) ; l'export
/// gardera les trois passes.
pub fn full_resolution(conn: &Connection, media_id: i64) -> Result<DevelopedRaw, SourceError> {
    Ok(open_raw(conn, media_id)?.develop_with(false, true)?)
}
