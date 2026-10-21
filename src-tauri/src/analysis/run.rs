//! Analyse d'une bibliothèque en arrière-plan : les photos jamais analysées,
//! ou analysées par une version plus ancienne. Les originaux sont lus, jamais
//! écrits.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use image::RgbImage;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::{SOURCE_SIDE, VERSION, VERSION_WITHOUT_EYES, analyze, eyes};
use crate::db::now_utc;
use crate::library::{LibraryError, get_library, pairs, paths};
use crate::metadata::media::format_of;
use crate::raw::preview;

/// Image d'une photo telle que le boîtier l'a prise : l'aperçu en cache
/// (au moins `min_side`) s'il vient du boîtier, sinon l'original. Un aperçu
/// retouché ne sert pas : on juge la prise de vue, pas la retouche.
pub fn source_image(
    conn: &Connection,
    cache_root: &Path,
    media_id: i64,
    min_side: u32,
) -> Option<RgbImage> {
    let size = preview::SIZES
        .iter()
        .copied()
        .find(|&s| s >= min_side)
        .unwrap_or(preview::SIZES[preview::SIZES.len() - 1]);
    let cached: Option<String> = conn
        .query_row(
            "SELECT source FROM previews WHERE media_id = ?1 AND size = ?2",
            params![media_id, size],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    if let Some(img) = cached
        .filter(|s| s != "rendered")
        .and_then(|_| std::fs::read(preview::cache_path(cache_root, media_id, size)).ok())
        .and_then(|data| preview::decode_jpeg_scaled(&data, min_side).ok())
    {
        return Some(img);
    }
    let (library_id, rel, orientation): (i64, String, u8) = conn
        .query_row(
            "SELECT library_id, path, orientation FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok()?;
    let lib = get_library(conn, library_id).ok()?;
    let abs = paths::to_absolute(&lib.root_path, &rel).ok()?;
    let format = format_of(&abs)?;
    preview::load(&abs, format, orientation, min_side).ok()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    pub analyzed: usize,
    /// Photos analysées signalées comme ratés probables.
    pub misses: usize,
    /// Photos illisibles pour l'instant (disque débranché, format sans aperçu).
    pub skipped: usize,
    /// Les yeux ont-ils été analysés (ONNX Runtime présent) ?
    pub eyes: bool,
}

/// Analyse les photos d'une bibliothèque (ou d'un album) qui ne l'ont pas
/// été par la version actuelle. `progress(fait, total)` suit l'avancement ;
/// `cancel` l'interrompt entre deux photos.
pub fn run(
    conn: &Connection,
    cache_root: &Path,
    library_id: i64,
    album_id: Option<i64>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize),
) -> Result<RunReport, LibraryError> {
    let detector = eyes::detector();
    let version = if detector.is_some() {
        VERSION
    } else {
        VERSION_WITHOUT_EYES
    };
    let ids: Vec<i64> = conn
        .prepare(&format!(
            "SELECT m.id FROM media m LEFT JOIN analysis a ON a.media_id = m.id
              WHERE m.library_id = ?1 AND (?2 IS NULL OR m.album_id = ?2)
                AND m.state = 'normal' AND m.kind = 'photo' AND {}
                AND (a.media_id IS NULL OR a.version < ?3)
              ORDER BY m.captured_at, m.id",
            pairs::HEAD_SQL
        ))?
        .query_map(params![library_id, album_id, version], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut report = RunReport {
        eyes: detector.is_some(),
        ..RunReport::default()
    };
    for (done, &id) in ids.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        match source_image(conn, cache_root, id, SOURCE_SIDE) {
            Some(img) => {
                let f = analyze(&img);
                let closed = detector.and_then(|d| d.lock().ok()?.eyes_closed(&img));
                conn.execute(
                    "INSERT OR REPLACE INTO analysis
                       (media_id, version, sharpness, blurry, exposure, eyes_closed, analyzed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        id,
                        version,
                        f.sharpness,
                        f.blurry,
                        f.exposure.map(|e| e.as_db()),
                        closed,
                        now_utc()
                    ],
                )?;
                report.analyzed += 1;
                report.misses +=
                    usize::from(f.blurry || f.exposure.is_some() || closed == Some(true));
            }
            None => report.skipped += 1,
        }
        progress(done + 1, ids.len());
    }
    Ok(report)
}
