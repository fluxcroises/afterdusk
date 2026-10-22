//! Image de travail d'une photo de la bibliothèque : un RAW est développé
//! par LibRaw, une photo déjà développée (JPEG, TIFF, PNG, HEIF) décodée
//! telle quelle (`rendered`). L'original n'est jamais écrit.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension};

use super::lens::Shot;
use super::pipeline::WorkingImage;
use super::rendered::{self, RenderedError};
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
    #[error("format non retouchable : {0}")]
    Unsupported(String),
    #[error(transparent)]
    Rendered(#[from] RenderedError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Raw(#[from] RawError),
}

/// Original d'une photo de la bibliothèque, ouvert en lecture seule.
enum Original {
    Raw(RawFile),
    /// Déjà développée : chemin et orientation EXIF.
    Rendered(PathBuf, u8),
}

fn open(conn: &Connection, media_id: i64) -> Result<Original, SourceError> {
    let (library_id, path, orientation): (i64, String, u8) = conn
        .query_row(
            "SELECT library_id, path, orientation FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or(SourceError::NotFound(media_id))?;
    let lib = get_library(conn, library_id)?;
    if !lib.is_online {
        return Err(SourceError::Offline(lib.root_path));
    }
    let abs = paths::to_absolute(&lib.root_path, &path)?;
    match format_of(&abs) {
        Some(MediaFormat::Raw) => Ok(Original::Raw(RawFile::open(&abs)?)),
        Some(MediaFormat::Image) => Ok(Original::Rendered(abs, orientation)),
        _ => Err(SourceError::Unsupported(path)),
    }
}

/// Décode l'original d'une photo et le ramène à `max_side` pixels de côté
/// long : en demi-définition (quatre fois plus rapide) quand elle suffit, en
/// pleine définition pour les capteurs trop petits.
pub fn working_image(
    conn: &Connection,
    media_id: i64,
    max_side: u32,
) -> Result<WorkingImage, SourceError> {
    let developed = match open(conn, media_id)? {
        Original::Raw(mut raw) => {
            let info = raw.info();
            let half = u32::from(info.width.max(info.height)) / 2 >= max_side;
            raw.develop(half)?
        }
        Original::Rendered(path, orientation) => rendered::develop(&path, orientation)?,
    };
    Ok(WorkingImage::from_raw(&developed, max_side))
}

/// Boîtier, objectif, focale et ouverture de la prise de vue, pour les
/// corrections d'objectif. Une photo déjà développée n'en reçoit pas : le
/// boîtier ou le logiciel qui l'a produite l'a souvent déjà corrigée, et les
/// réglages à zéro doivent la redonner telle quelle.
pub fn shot(conn: &Connection, media_id: i64) -> Result<Shot, SourceError> {
    match open(conn, media_id)? {
        Original::Raw(raw) => Ok(Shot::from(&raw.info())),
        Original::Rendered(..) => Ok(Shot::default()),
    }
}

/// Original développé en pleine définition, pour le zoom 100 %. Le X-Trans
/// y est dématricé en une passe (5 s au lieu de 11 s pour 26 Mpx) ; l'export
/// gardera les trois passes.
pub fn full_resolution(conn: &Connection, media_id: i64) -> Result<DevelopedRaw, SourceError> {
    match open(conn, media_id)? {
        Original::Raw(mut raw) => Ok(raw.develop_with(false, true)?),
        Original::Rendered(path, orientation) => Ok(rendered::develop(&path, orientation)?),
    }
}
