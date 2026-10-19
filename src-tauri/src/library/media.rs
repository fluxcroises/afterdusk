//! Lecture des médias et des albums pour les grilles et la fiche d'informations,
//! et index du cache d'aperçus.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::{LibraryError, get_library, paths};
use crate::db::now_utc;
use crate::metadata::media::{MediaFormat, format_of};
use crate::raw::preview::{self, Generated, PreviewError, SIZES};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSummary {
    pub id: i64,
    pub album_id: i64,
    pub file_name: String,
    pub captured_at: Option<String>,
    pub rating: i64,
    pub flag: i64,
    pub color: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub orientation: i64,
    /// Les trois paliers d'aperçu sont-ils en cache ?
    pub has_preview: bool,
}

/// Médias d'une bibliothèque (ou d'un album), par date de prise de vue.
pub fn list_media(
    conn: &Connection,
    library_id: i64,
    album_id: Option<i64>,
) -> Result<Vec<MediaSummary>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.album_id, m.path, m.captured_at, m.rating, m.flag, m.color,
                m.width, m.height, m.orientation,
                (SELECT count(*) FROM previews p WHERE p.media_id = m.id) = 3
         FROM media m
         WHERE m.library_id = ?1 AND m.state = 'normal' AND (?2 IS NULL OR m.album_id = ?2)
         ORDER BY m.captured_at, m.path",
    )?;
    let rows = stmt
        .query_map(params![library_id, album_id], |r| {
            let path: String = r.get(2)?;
            Ok(MediaSummary {
                id: r.get(0)?,
                album_id: r.get(1)?,
                file_name: path.rsplit('/').next().unwrap_or(&path).to_owned(),
                captured_at: r.get(3)?,
                rating: r.get(4)?,
                flag: r.get(5)?,
                color: r.get(6)?,
                width: r.get(7)?,
                height: r.get(8)?,
                orientation: r.get(9)?,
                has_preview: r.get(10)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub id: i64,
    pub name: String,
    pub folder: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub count: i64,
    /// Couverture choisie, sinon la première photo de l'album.
    pub cover_media_id: Option<i64>,
    /// La couverture a-t-elle été choisie par l'utilisateur ?
    pub cover_chosen: bool,
}

pub fn list_albums(conn: &Connection, library_id: i64) -> Result<Vec<AlbumSummary>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.folder, a.start_date, a.end_date,
                (SELECT count(*) FROM media m WHERE m.album_id = a.id AND m.state = 'normal'),
                coalesce(a.cover_media_id,
                         (SELECT m.id FROM media m WHERE m.album_id = a.id AND m.state = 'normal'
                          ORDER BY m.captured_at, m.path LIMIT 1)),
                a.cover_media_id IS NOT NULL
         FROM albums a WHERE a.library_id = ?1
         ORDER BY a.start_date DESC, a.name",
    )?;
    let rows = stmt
        .query_map([library_id], |r| {
            Ok(AlbumSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                folder: r.get(2)?,
                start_date: r.get(3)?,
                end_date: r.get(4)?,
                count: r.get(5)?,
                cover_media_id: r.get(6)?,
                cover_chosen: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Photo pouvant illustrer l'accueil.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub media_id: i64,
    pub album_name: String,
    pub captured_at: Option<String>,
}

/// Meilleures photos de la bibliothèque pour l'illustration de l'accueil :
/// retenues et mieux notées d'abord, puis les plus récentes, avec un aperçu
/// 2048 px en cache. Les rejetées sont exclues.
pub fn highlights(
    conn: &Connection,
    library_id: i64,
    limit: usize,
) -> Result<Vec<Highlight>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT m.id, a.name, m.captured_at FROM media m
         JOIN albums a ON a.id = m.album_id
         WHERE m.library_id = ?1 AND m.state = 'normal' AND m.flag >= 0
           AND EXISTS (SELECT 1 FROM previews p WHERE p.media_id = m.id AND p.size = 2048)
         ORDER BY m.flag DESC, m.rating DESC, m.captured_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![library_id, limit as i64], |r| {
            Ok(Highlight {
                media_id: r.get(0)?,
                album_name: r.get(1)?,
                captured_at: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Fiche d'informations d'une photo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDetails {
    pub id: i64,
    pub file_name: String,
    pub original_name: String,
    pub path: String,
    pub captured_at: Option<String>,
    pub utc_offset: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<f64>,
    pub focal_length_35mm: Option<i64>,
    pub aperture: Option<f64>,
    pub shutter: Option<f64>,
    pub iso: Option<f64>,
    pub exposure_compensation: Option<f64>,
    pub keywords: Vec<String>,
}

/// Séparateur des mots-clés dans `group_concat` (caractère de contrôle US).
const KEYWORD_SEPARATOR: char = 31u8 as char;

pub fn media_details(conn: &Connection, id: i64) -> Result<Option<MediaDetails>, LibraryError> {
    Ok(conn
        .query_row(
            "SELECT m.id, m.path, m.original_name, m.captured_at, m.utc_offset, m.width, m.height,
                    trim(coalesce(c.make, '') || ' ' || coalesce(c.model, '')), l.model,
                    e.focal_length, e.focal_length_35mm, e.aperture, e.shutter, e.iso,
                    e.exposure_compensation,
                    (SELECT group_concat(k.word, char(31)) FROM media_keywords mk
                       JOIN keywords k ON k.id = mk.keyword_id WHERE mk.media_id = m.id)
             FROM media m
             LEFT JOIN gear c ON c.id = m.camera_id
             LEFT JOIN gear l ON l.id = m.lens_id
             LEFT JOIN media_exif e ON e.media_id = m.id
             WHERE m.id = ?1",
            [id],
            |r| {
                let path: String = r.get(1)?;
                Ok(MediaDetails {
                    id: r.get(0)?,
                    file_name: path.rsplit('/').next().unwrap_or(&path).to_owned(),
                    path,
                    original_name: r.get(2)?,
                    captured_at: r.get(3)?,
                    utc_offset: r.get(4)?,
                    width: r.get(5)?,
                    height: r.get(6)?,
                    camera: r.get::<_, Option<String>>(7)?.filter(|s| !s.is_empty()),
                    lens: r.get(8)?,
                    focal_length: r.get(9)?,
                    focal_length_35mm: r.get(10)?,
                    aperture: r.get(11)?,
                    shutter: r.get(12)?,
                    iso: r.get(13)?,
                    exposure_compensation: r.get(14)?,
                    keywords: r
                        .get::<_, Option<String>>(15)?
                        .map(|k| k.split(KEYWORD_SEPARATOR).map(str::to_owned).collect())
                        .unwrap_or_default(),
                })
            },
        )
        .optional()?)
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Path(#[from] paths::PathError),
    #[error(transparent)]
    Preview(#[from] PreviewError),
    #[error("média {0} introuvable")]
    NotFound(i64),
    #[error("bibliothèque hors ligne : {0}")]
    Offline(PathBuf),
    #[error("format sans aperçu : {0}")]
    UnknownFormat(String),
}

/// Génère et indexe les aperçus d'un média, depuis l'original dans la bibliothèque.
pub fn generate_previews(
    conn: &Connection,
    cache_root: &Path,
    media_id: i64,
) -> Result<Vec<Generated>, CacheError> {
    let (library_id, path, orientation): (i64, String, u8) = conn
        .query_row(
            "SELECT library_id, path, orientation FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or(CacheError::NotFound(media_id))?;
    let lib = get_library(conn, library_id)?;
    if !lib.is_online {
        return Err(CacheError::Offline(lib.root_path));
    }
    let abs = paths::to_absolute(&lib.root_path, &path)?;
    let format = format_of(&abs).ok_or_else(|| CacheError::UnknownFormat(path.clone()))?;
    let generated = preview::generate(&abs, format, orientation, cache_root, media_id)?;
    let source = match format {
        MediaFormat::Raw => "embedded",
        MediaFormat::Image => "decoded",
    };
    let now = now_utc();
    for g in &generated {
        conn.execute(
            "INSERT OR REPLACE INTO previews
               (media_id, size, width, height, bytes, source, created_at, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                media_id,
                g.size,
                g.width,
                g.height,
                g.bytes as i64,
                source,
                now
            ],
        )?;
    }
    Ok(generated)
}

/// JPEG pleine définition pour le zoom 100 % : le plus grand aperçu intégré
/// au RAW (extrait sans décodage), ou le fichier lui-même pour un JPEG.
/// L'original est ouvert en lecture seule. `None` si la bibliothèque est
/// débranchée ou si le format n'en fournit pas : l'interface garde alors
/// l'aperçu 2048 px. L'orientation n'est pas appliquée (voir `orientation`).
pub fn full_size_jpeg(conn: &Connection, media_id: i64) -> Result<Option<Vec<u8>>, CacheError> {
    let (library_id, path): (i64, String) = conn
        .query_row(
            "SELECT library_id, path FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(CacheError::NotFound(media_id))?;
    let lib = get_library(conn, library_id)?;
    if !lib.is_online {
        return Ok(None);
    }
    let abs = paths::to_absolute(&lib.root_path, &path)?;
    let is_jpeg_file = abs
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"));
    match format_of(&abs) {
        Some(MediaFormat::Raw) => {
            let mut raw = crate::raw::RawFile::open(&abs).map_err(PreviewError::from)?;
            let preview = raw.preview().map_err(PreviewError::from)?;
            Ok((preview.format == crate::raw::PreviewFormat::Jpeg).then_some(preview.data))
        }
        Some(MediaFormat::Image) if is_jpeg_file => std::fs::read(&abs)
            .map(Some)
            .map_err(|source| PreviewError::Io { path: abs, source }.into()),
        _ => Ok(None),
    }
}

/// Médias dont un palier manque (import interrompu, cache vidé).
pub fn missing_previews(conn: &Connection) -> Result<Vec<i64>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT m.id FROM media m
         WHERE m.state = 'normal'
           AND (SELECT count(*) FROM previews p WHERE p.media_id = m.id) < ?1
         ORDER BY m.imported_at DESC",
    )?;
    let ids = stmt
        .query_map([SIZES.len() as i64], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Fichier d'aperçu en cache, s'il existe.
pub fn cached_preview(cache_root: &Path, media_id: i64, size: u32) -> Option<PathBuf> {
    let path = preview::cache_path(cache_root, media_id, size);
    path.is_file().then_some(path)
}
