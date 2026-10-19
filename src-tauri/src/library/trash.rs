//! Corbeille interne (cahier des charges, section 4.4).
//!
//! Envoyer une photo à la corbeille la **déplace**, avec son XMP, dans
//! `<bibliothèque>/.afterdusk-corbeille/<id>/` ; son emplacement d'origine est
//! noté pour la restaurer. Le vidage (manuel, ou après le délai réglé) est la
//! seule suppression définitive d'Afterdusk : il ne touche qu'au dossier de la
//! corbeille, jamais à la copie de sauvegarde.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::{Library, LibraryError, get_library, paths};
use crate::db::now_utc;
use crate::raw::preview::{SIZES, cache_path};

/// Dossier de la corbeille, à la racine de la bibliothèque (caché : il n'est
/// ni analysé comme source d'import ni affiché parmi les albums).
pub const TRASH_DIR: &str = ".afterdusk-corbeille";

/// Délai par défaut avant suppression définitive.
pub const DEFAULT_RETENTION_DAYS: u32 = 30;

#[derive(Debug, thiserror::Error)]
pub enum TrashError {
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Path(#[from] paths::PathError),
    #[error("la bibliothèque est hors ligne : {0}")]
    Offline(PathBuf),
    #[error("média {0} introuvable")]
    NotFound(i64),
    #[error("déplacement impossible de {path} : {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("refus de supprimer {0} : hors de la corbeille")]
    OutsideTrash(PathBuf),
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashReport {
    pub done: usize,
    pub failed: usize,
    /// Photos réellement déplacées, pour l'annulation (Ctrl+Z).
    pub moved: Vec<i64>,
}

struct Entry {
    id: i64,
    library_id: i64,
    path: String,
    sidecar: Option<String>,
    origin: Option<String>,
}

fn entry(conn: &Connection, id: i64) -> Result<Entry, TrashError> {
    conn.query_row(
        "SELECT id, library_id, path, sidecar_path, trash_origin_path FROM media WHERE id = ?1",
        [id],
        |r| {
            Ok(Entry {
                id: r.get(0)?,
                library_id: r.get(1)?,
                path: r.get(2)?,
                sidecar: r.get(3)?,
                origin: r.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or(TrashError::NotFound(id))
}

fn online(conn: &Connection, library_id: i64) -> Result<Library, TrashError> {
    let lib = get_library(conn, library_id)?;
    if lib.is_online {
        Ok(lib)
    } else {
        Err(TrashError::Offline(lib.root_path))
    }
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> TrashError + '_ {
    move |source| TrashError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Déplace un fichier sans jamais écraser la destination.
fn move_file(from: &Path, to: &Path) -> Result<(), TrashError> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(io(parent))?;
    }
    if to.exists() {
        return Err(TrashError::Io {
            path: to.to_path_buf(),
            source: std::io::Error::from(std::io::ErrorKind::AlreadyExists),
        });
    }
    fs::rename(from, to).map_err(io(from))
}

/// Nom du XMP annexe à l'emplacement `rel`, en gardant sa convention de nommage.
fn sidecar_for(old_media: &str, old_sidecar: &str, new_media: &str) -> String {
    // `IMG.xmp` (RAW) ou `IMG.JPG.xmp` (autres) : on remplace seulement le dossier
    // et la partie qui vient du nom du média.
    let old_name = old_media.rsplit('/').next().unwrap_or(old_media);
    let new_name = new_media.rsplit('/').next().unwrap_or(new_media);
    let side_name = old_sidecar.rsplit('/').next().unwrap_or(old_sidecar);
    let stem = |n: &str| n.rsplit_once('.').map_or(n, |(s, _)| s).to_owned();
    let renamed = if side_name == format!("{old_name}.xmp") {
        format!("{new_name}.xmp")
    } else {
        format!("{}.xmp", stem(new_name))
    };
    match new_media.rsplit_once('/') {
        Some((folder, _)) => format!("{folder}/{renamed}"),
        None => renamed,
    }
}

/// Déplace le média et son XMP ; en cas d'échec du XMP, le média revient.
fn relocate(root: &Path, e: &Entry, new_path: &str) -> Result<Option<String>, TrashError> {
    let from = paths::to_absolute(root, &e.path)?;
    let to = paths::to_absolute(root, new_path)?;
    move_file(&from, &to)?;
    let Some(sidecar) = &e.sidecar else {
        return Ok(None);
    };
    let new_sidecar = sidecar_for(&e.path, sidecar, new_path);
    let side_from = paths::to_absolute(root, sidecar)?;
    if side_from.exists() {
        let side_to = paths::to_absolute(root, &new_sidecar)?;
        if let Err(err) = move_file(&side_from, &side_to) {
            let _ = fs::rename(&to, &from);
            return Err(err);
        }
    }
    Ok(Some(new_sidecar))
}

/// Envoie des photos à la corbeille.
pub fn move_to_trash(conn: &mut Connection, ids: &[i64]) -> Result<TrashReport, TrashError> {
    let mut report = TrashReport::default();
    for &id in ids {
        let e = entry(conn, id)?;
        if e.origin.is_some() {
            continue; // déjà dans la corbeille
        }
        let lib = online(conn, e.library_id)?;
        let name = e.path.rsplit('/').next().unwrap_or(&e.path).to_owned();
        let trashed = format!("{TRASH_DIR}/{}/{name}", e.id);
        match relocate(&lib.root_path, &e, &trashed) {
            Ok(sidecar) => {
                conn.execute(
                    "UPDATE media SET state = 'trashed', trashed_at = ?2, trash_origin_path = ?3,
                            path = ?4, sidecar_path = ?5, updated_at = ?2
                     WHERE id = ?1",
                    params![id, now_utc(), e.path, trashed, sidecar],
                )?;
                report.done += 1;
                report.moved.push(id);
            }
            Err(err) => {
                eprintln!("corbeille, média {id} : {err}");
                report.failed += 1;
            }
        }
    }
    // Un dessus de pile parti seul : le fichier resté prend sa place.
    for &id in ids {
        super::pairs::promote_after_leaving(conn, id)?;
    }
    Ok(report)
}

/// Remet des photos à leur emplacement d'origine (un autre nom si la place est prise).
pub fn restore(conn: &mut Connection, ids: &[i64]) -> Result<TrashReport, TrashError> {
    let mut report = TrashReport::default();
    for &id in ids {
        let e = entry(conn, id)?;
        let Some(origin) = e.origin.clone() else {
            continue;
        };
        let lib = online(conn, e.library_id)?;
        let target = free_path(&lib.root_path, &origin)?;
        match relocate(&lib.root_path, &e, &target) {
            Ok(sidecar) => {
                conn.execute(
                    "UPDATE media SET state = 'normal', trashed_at = NULL, trash_origin_path = NULL,
                            path = ?2, sidecar_path = ?3, updated_at = ?4
                     WHERE id = ?1",
                    params![id, target, sidecar, now_utc()],
                )?;
                remove_empty_dir(&lib.root_path, id);
                report.done += 1;
            }
            Err(err) => {
                eprintln!("restauration, média {id} : {err}");
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

/// `origin`, ou `nom-2.ext`, `nom-3.ext`… si un fichier occupe déjà la place.
fn free_path(root: &Path, origin: &str) -> Result<String, TrashError> {
    let (folder, name) = origin.rsplit_once('/').unwrap_or(("", origin));
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_owned(), format!(".{e}")),
        _ => (name.to_owned(), String::new()),
    };
    for n in 1..=10_000 {
        let candidate_name = if n == 1 {
            name.to_owned()
        } else {
            format!("{stem}-{n}{ext}")
        };
        let candidate = if folder.is_empty() {
            candidate_name
        } else {
            format!("{folder}/{candidate_name}")
        };
        if !paths::to_absolute(root, &candidate)?.exists() {
            return Ok(candidate);
        }
    }
    Err(TrashError::Io {
        path: root.join(origin),
        source: std::io::Error::from(std::io::ErrorKind::AlreadyExists),
    })
}

fn trash_root(root: &Path) -> PathBuf {
    root.join(TRASH_DIR)
}

/// Garde-fou : n'efface que ce qui se trouve dans le dossier de la corbeille.
fn ensure_in_trash(root: &Path, path: &Path) -> Result<(), TrashError> {
    let trash = trash_root(root);
    let inside = path.starts_with(&trash)
        && path
            .strip_prefix(&trash)
            .is_ok_and(|rel| rel.components().count() >= 1);
    if inside {
        Ok(())
    } else {
        Err(TrashError::OutsideTrash(path.to_path_buf()))
    }
}

fn remove_empty_dir(root: &Path, id: i64) {
    let _ = fs::remove_dir(trash_root(root).join(id.to_string()));
}

/// Supprime définitivement des photos de la corbeille : fichiers, aperçus en
/// cache et fiches. Les photos hors de la corbeille sont ignorées.
pub fn purge(
    conn: &mut Connection,
    cache_root: &Path,
    ids: &[i64],
) -> Result<TrashReport, TrashError> {
    let mut report = TrashReport::default();
    for &id in ids {
        let e = entry(conn, id)?;
        if e.origin.is_none() {
            continue; // pas dans la corbeille : jamais supprimée
        }
        let lib = online(conn, e.library_id)?;
        let mut files = vec![paths::to_absolute(&lib.root_path, &e.path)?];
        if let Some(sidecar) = &e.sidecar {
            files.push(paths::to_absolute(&lib.root_path, sidecar)?);
        }
        let mut ok = true;
        for file in &files {
            ensure_in_trash(&lib.root_path, file)?;
            if file.exists()
                && let Err(err) = fs::remove_file(file)
            {
                eprintln!("vidage, {} : {err}", file.display());
                ok = false;
            }
        }
        if !ok {
            report.failed += 1;
            continue;
        }
        remove_empty_dir(&lib.root_path, id);
        for size in SIZES {
            let _ = fs::remove_file(cache_path(cache_root, id, size));
        }
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM media_search WHERE rowid = ?1", [id])?;
        tx.execute("DELETE FROM media WHERE id = ?1", [id])?;
        tx.commit()?;
        report.done += 1;
    }
    Ok(report)
}

/// Photos de la corbeille d'une bibliothèque, éventuellement plus anciennes que
/// `older_than_days`.
pub fn trashed_ids(
    conn: &Connection,
    library_id: Option<i64>,
    older_than_days: Option<u32>,
) -> Result<Vec<i64>, TrashError> {
    let cutoff = older_than_days.map(|d| {
        (chrono::Utc::now() - chrono::Duration::days(i64::from(d)))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    });
    let mut stmt = conn.prepare(
        "SELECT id FROM media WHERE state = 'trashed'
           AND (?1 IS NULL OR library_id = ?1)
           AND (?2 IS NULL OR trashed_at <= ?2)
         ORDER BY trashed_at",
    )?;
    let ids = stmt
        .query_map(params![library_id, cutoff], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Photos de la corbeille comme les montre la grille : une pile compte pour une.
pub fn trashed_count(conn: &Connection, library_id: i64) -> Result<usize, TrashError> {
    let n: i64 = conn.query_row(
        &format!(
            "SELECT count(*) FROM media m WHERE m.library_id = ?1 AND m.state = 'trashed' AND {}",
            super::pairs::HEAD_SQL
        ),
        [library_id],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

/// Vidage automatique au démarrage : bibliothèques branchées seulement.
pub fn purge_expired(
    conn: &mut Connection,
    cache_root: &Path,
    retention_days: u32,
) -> Result<TrashReport, TrashError> {
    let mut report = TrashReport::default();
    for lib in super::list_libraries(conn)? {
        if !lib.is_online {
            continue;
        }
        let ids = trashed_ids(conn, Some(lib.id), Some(retention_days))?;
        let r = purge(conn, cache_root, &ids)?;
        report.done += r.done;
        report.failed += r.failed;
    }
    Ok(report)
}

/// Délai de la corbeille, en jours (réglage `trash.retentionDays`).
pub fn retention_days(conn: &Connection) -> Result<u32, TrashError> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'trash.retentionDays'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(value
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|d| (1..=365).contains(d))
        .unwrap_or(DEFAULT_RETENTION_DAYS))
}

pub fn set_retention_days(conn: &Connection, days: u32) -> Result<(), TrashError> {
    let days = days.clamp(1, 365);
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('trash.retentionDays', ?1)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [days.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nommage_du_xmp_conserve() {
        assert_eq!(
            sidecar_for(
                "2026/a/IMG.RAF",
                "2026/a/IMG.xmp",
                ".afterdusk-corbeille/7/IMG.RAF"
            ),
            ".afterdusk-corbeille/7/IMG.xmp"
        );
        assert_eq!(
            sidecar_for("2026/a/IMG.JPG", "2026/a/IMG.JPG.xmp", "2026/a/IMG-2.JPG"),
            "2026/a/IMG-2.JPG.xmp"
        );
    }

    #[test]
    fn garde_fou_hors_corbeille() {
        let root = Path::new("bib");
        assert!(ensure_in_trash(root, &root.join(TRASH_DIR).join("1").join("a.RAF")).is_ok());
        assert!(ensure_in_trash(root, &root.join("2026").join("a.RAF")).is_err());
        assert!(ensure_in_trash(root, &root.join(TRASH_DIR)).is_err());
    }
}
