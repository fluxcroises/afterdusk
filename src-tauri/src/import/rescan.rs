//! Réanalyse d'une bibliothèque (cahier des charges, section 5) : les fichiers
//! présents dans ses dossiers mais absents de la base y sont inscrits, avec
//! leur album (le dossier), leurs marques et mots-clés, et leurs retouches,
//! instantanés et copies virtuelles repris du XMP. C'est ainsi qu'une base
//! perdue se reconstruit. Rien n'est écrit sur le disque : ni les originaux,
//! ni les XMP.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;
use serde::Serialize;
use walkdir::WalkDir;

use super::copy::{PART_SUFFIX, hash_file};
use super::{ImportError, NewMedia, insert_media, scan, sidecar_naming};
use crate::develop::store;
use crate::library::pairs::{self, Candidate};
use crate::library::{get_library, paths};
use crate::metadata::media::{MediaFormat, format_of};
use crate::metadata::xmp;

/// Bilan d'une réanalyse.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RescanReport {
    /// Fichiers inscrits en base.
    pub added: usize,
    /// Photos dont les retouches ont été reprises du XMP.
    pub restored: usize,
    /// Fichiers illisibles : chemin relatif et raison.
    pub failed: Vec<(String, String)>,
    /// Médias ajoutés, pour leurs aperçus.
    #[serde(skip)]
    pub media_ids: Vec<i64>,
}

/// Dossier ou fichier ignoré : caché, temporaire d'Afterdusk.
fn ignored(name: &str) -> bool {
    name.starts_with('.') || name.ends_with(PART_SUFFIX)
}

/// Réanalyse la bibliothèque `library_id` ; `progress(fait, total)` après
/// chaque fichier. L'arrêt (`cancel`) laisse les fichiers déjà inscrits.
pub fn rescan(
    conn: &mut Connection,
    library_id: i64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize),
) -> Result<RescanReport, ImportError> {
    let lib = get_library(conn, library_id)?;
    if !lib.is_online {
        return Err(ImportError::LibraryOffline(lib.root_path.clone()));
    }
    let known: HashSet<String> = conn
        .prepare("SELECT path FROM media WHERE library_id = ?1")?
        .query_map([library_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;

    // Fichiers de média absents de la base.
    let mut found = Vec::new();
    for entry in WalkDir::new(&lib.root_path)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !ignored(&e.file_name().to_string_lossy()))
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(format) = format_of(path) else {
            continue;
        };
        let Ok(rel) = paths::to_relative(&lib.root_path, path) else {
            continue;
        };
        if !known.contains(&rel) {
            found.push((path.to_path_buf(), rel, format));
        }
    }
    found.sort_by(|a, b| a.1.cmp(&b.1));

    let total = found.len();
    let mut report = RescanReport::default();
    let mut added: Vec<(String, MediaFormat, String, i64)> = Vec::new();
    for (index, (abs, rel, format)) in found.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        match add_one(conn, &lib, abs, rel, *format) {
            Ok((media_id, captured_at, restored)) => {
                report.added += 1;
                report.restored += usize::from(restored);
                report.media_ids.push(media_id);
                added.push((rel.clone(), *format, captured_at, media_id));
            }
            Err(e) => report.failed.push((rel.clone(), e)),
        }
        progress(index + 1, total);
    }

    // Piles RAW + JPEG parmi les fichiers ajoutés.
    let candidates: Vec<Candidate> = added
        .iter()
        .map(|(rel, format, at, _)| Candidate {
            path: Path::new(rel),
            format: *format,
            captured_at: Some(at.as_str()),
        })
        .collect();
    for (raw, companion) in pairs::find_pairs(&candidates) {
        pairs::link(conn, added[raw].3, added[companion].3)?;
    }
    Ok(report)
}

/// Inscrit un fichier ; renvoie le média, sa date de prise de vue et si des
/// retouches ont été reprises du XMP.
fn add_one(
    conn: &mut Connection,
    lib: &crate::library::Library,
    abs: &Path,
    rel: &str,
    format: MediaFormat,
) -> Result<(i64, String, bool), String> {
    let file = scan::describe(abs, format).map_err(|e| e.to_string())?;
    let hash = hash_file(abs).map_err(|e| e.to_string())?;
    let sidecar_abs = xmp::sidecar_path(abs, sidecar_naming(format));
    // XMP illisible : le fichier est inscrit sans ses marques.
    let marks = xmp::read_sidecar(&sidecar_abs)
        .ok()
        .flatten()
        .unwrap_or_default();
    let sidecar = xmp::sidecar_path(Path::new(rel), sidecar_naming(format))
        .to_string_lossy()
        .replace('\\', "/");
    let (folder, file_name) = rel.rsplit_once('/').unwrap_or(("", rel));
    let album_name = folder.rsplit('/').next().unwrap_or(folder);
    let album_name = if album_name.is_empty() {
        lib.name.as_str()
    } else {
        album_name
    };
    let original_name = marks
        .original_file_name
        .clone()
        .unwrap_or_else(|| file_name.to_owned());
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let media_id = insert_media(
        &tx,
        lib,
        &NewMedia {
            album_name,
            folder,
            file_name,
            target: rel,
            sidecar: &sidecar,
            original_name: &original_name,
            fingerprint: &file.fingerprint,
            hash: &hash,
            import_id: None,
            file: &file,
            marks: &marks,
        },
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    let restored = store::restore_from_xmp(
        conn,
        media_id,
        marks.develop.as_deref(),
        marks.snapshots.as_deref(),
        marks.versions.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    Ok((media_id, file.captured_at.clone(), restored))
}
