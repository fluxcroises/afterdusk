//! Dossiers surveillés (cahier des charges, 4.1 bis ; décision 0009) : le
//! logiciel d'un boîtier (connexion USB ou Wi-Fi) ou un transfert sans fil
//! y dépose les photos. Afterdusk attend qu'un fichier soit complet (taille
//! stable), le copie avec les garanties de l'import (somme de contrôle,
//! sauvegarde) dans l'album choisi, et ne modifie jamais le dossier.
//!
//! Ce module tient la liste des dossiers et la logique pure (fichiers
//! arrivés, stabilité, import) ; le fil de surveillance vit dans
//! `commands::watch`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use walkdir::WalkDir;

use super::auto::{self, AutoSettings};
use super::{ImportError, ImportOptions, ImportProgress, known_fingerprints, scan};
use crate::db::now_utc;
use crate::library;
use crate::metadata::media::format_of;

/// Taille inchangée pendant ce délai : le fichier est complet.
pub const STABLE: Duration = Duration::from_millis(800);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedFolder {
    pub id: i64,
    pub path: PathBuf,
    pub library_id: i64,
    pub library_name: String,
    pub album_name: String,
    pub settings: AutoSettings,
    pub enabled: bool,
    /// Activation, au millième de seconde (UTC).
    pub active_since: String,
    /// Fichiers présents à l'activation (chemins relatifs au dossier) : ignorés.
    #[serde(skip)]
    pub existing: HashSet<String>,
}

impl WatchedFolder {
    /// Chemin relatif au dossier, quelle que soit la forme donnée par le
    /// système (`/var` ou `/private/var` sous macOS).
    fn relative(&self, path: &Path) -> Option<String> {
        let rel = path
            .strip_prefix(&self.path)
            .ok()
            .map(Path::to_path_buf)
            .or_else(|| {
                let canon = std::fs::canonicalize(&self.path).ok()?;
                let file = std::fs::canonicalize(path).ok()?;
                file.strip_prefix(canon).ok().map(Path::to_path_buf)
            })?;
        Some(rel.to_string_lossy().replace('\\', "/"))
    }
}

fn now_precise() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Même dossier, sans dépendre de la casse ni des séparateurs sous Windows.
fn same_or_inside(path: &Path, root: &Path) -> bool {
    let norm = |p: &Path| {
        let canon = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let text = canon.to_string_lossy().replace('\\', "/");
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text
        }
    };
    let (p, r) = (norm(path), norm(root));
    let r = r.trim_end_matches('/');
    p == r || p.starts_with(&format!("{r}/"))
}

/// Refuse un dossier absent, ou mêlé à une bibliothèque : Afterdusk
/// s'importerait lui-même en boucle.
fn check(conn: &Connection, path: &Path, except: Option<i64>) -> Result<(), ImportError> {
    if !path.is_dir() {
        return Err(ImportError::WatchFolder("missing", path.to_path_buf()));
    }
    for lib in library::list_libraries(conn)? {
        if same_or_inside(path, &lib.root_path) || same_or_inside(&lib.root_path, path) {
            return Err(ImportError::WatchFolder(
                "insideLibrary",
                path.to_path_buf(),
            ));
        }
    }
    for folder in list(conn)? {
        if Some(folder.id) != except && same_or_inside(path, &folder.path) {
            return Err(ImportError::WatchFolder("duplicate", path.to_path_buf()));
        }
    }
    Ok(())
}

pub fn list(conn: &Connection) -> Result<Vec<WatchedFolder>, ImportError> {
    let rows = conn
        .prepare(
            "SELECT w.id, w.path, w.library_id, l.name, w.album_name, w.settings, w.enabled,
                    w.active_since
               FROM watched_folders w JOIN libraries l ON l.id = w.library_id
              ORDER BY w.path",
        )?
        .query_map([], |r| {
            Ok(WatchedFolder {
                id: r.get(0)?,
                path: PathBuf::from(r.get::<_, String>(1)?),
                library_id: r.get(2)?,
                library_name: r.get(3)?,
                album_name: r.get(4)?,
                settings: auto::decode(&r.get::<_, String>(5)?),
                enabled: r.get(6)?,
                active_since: r.get(7)?,
                existing: HashSet::new(),
            })
        })?
        .collect::<Result<Vec<WatchedFolder>, _>>()?;
    let mut rows = rows;
    let mut stmt = conn.prepare("SELECT path FROM watched_existing WHERE folder_id = ?1")?;
    for folder in &mut rows {
        folder.existing = stmt
            .query_map([folder.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
    }
    Ok(rows)
}

/// Médias du dossier, chemins relatifs.
fn media_files(path: &Path) -> Vec<String> {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !hidden(e.path()))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && format_of(e.path()).is_some())
        .filter_map(|e| {
            e.path()
                .strip_prefix(path)
                .ok()
                .map(|r| r.to_string_lossy().replace('\\', "/"))
        })
        .collect()
}

/// Relève les fichiers présents : ils seront ignorés.
fn snapshot(conn: &Connection, id: i64, path: &Path) -> Result<(), ImportError> {
    conn.execute("DELETE FROM watched_existing WHERE folder_id = ?1", [id])?;
    let mut insert =
        conn.prepare("INSERT OR IGNORE INTO watched_existing (folder_id, path) VALUES (?1, ?2)")?;
    for rel in media_files(path) {
        insert.execute(params![id, rel])?;
    }
    Ok(())
}

/// Ajoute un dossier, actif aussitôt ; ce qu'il contient déjà est ignoré.
pub fn add(
    conn: &Connection,
    path: &Path,
    library_id: i64,
    album_name: &str,
    settings: &AutoSettings,
) -> Result<i64, ImportError> {
    check(conn, path, None)?;
    library::get_library(conn, library_id)?;
    let album = album_name.trim();
    if album.is_empty() {
        return Err(ImportError::Library(library::LibraryError::EmptyName));
    }
    conn.execute(
        "INSERT INTO watched_folders
           (path, library_id, album_name, settings, enabled, active_since, created_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
        params![
            path.to_string_lossy(),
            library_id,
            album,
            auto::encode(settings),
            now_precise(),
            now_utc()
        ],
    )?;
    let id = conn.last_insert_rowid();
    snapshot(conn, id, path)?;
    Ok(id)
}

pub fn update(
    conn: &Connection,
    id: i64,
    album_name: &str,
    settings: &AutoSettings,
) -> Result<(), ImportError> {
    let album = album_name.trim();
    if album.is_empty() {
        return Err(ImportError::Library(library::LibraryError::EmptyName));
    }
    if conn.execute(
        "UPDATE watched_folders SET album_name = ?2, settings = ?3 WHERE id = ?1",
        params![id, album, auto::encode(settings)],
    )? == 0
    {
        return Err(ImportError::NotFound(id));
    }
    Ok(())
}

/// Active ou suspend ; une réactivation ignore ce qui est arrivé entre-temps.
pub fn set_enabled(conn: &Connection, id: i64, enabled: bool) -> Result<(), ImportError> {
    let changed = if enabled {
        let changed = conn.execute(
            "UPDATE watched_folders SET enabled = 1, active_since = ?2
              WHERE id = ?1 AND enabled = 0",
            params![id, now_precise()],
        )?;
        if changed > 0 {
            let path: String = conn.query_row(
                "SELECT path FROM watched_folders WHERE id = ?1",
                [id],
                |r| r.get(0),
            )?;
            snapshot(conn, id, Path::new(&path))?;
        }
        changed
    } else {
        conn.execute("UPDATE watched_folders SET enabled = 0 WHERE id = ?1", [id])?
    };
    if changed == 0 {
        let exists: Option<i64> = conn
            .query_row("SELECT id FROM watched_folders WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        exists.ok_or(ImportError::NotFound(id))?;
    }
    Ok(())
}

/// Dossier de réception FTP, tenu à jour d'après les réglages de la
/// réception : créé au besoin, actif ou suspendu avec elle.
pub fn ensure_reception(
    conn: &Connection,
    path: &Path,
    library_id: i64,
    album_name: &str,
    settings: &AutoSettings,
    enabled: bool,
) -> Result<i64, ImportError> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM watched_folders WHERE path = ?1",
            [path.to_string_lossy()],
            |r| r.get(0),
        )
        .optional()?;
    let album = if album_name.trim().is_empty() {
        "FTP"
    } else {
        album_name.trim()
    };
    let id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE watched_folders SET library_id = ?2, album_name = ?3, settings = ?4
                  WHERE id = ?1",
                params![id, library_id, album, auto::encode(settings)],
            )?;
            id
        }
        None => {
            std::fs::create_dir_all(path)
                .map_err(|_| ImportError::WatchFolder("missing", path.to_path_buf()))?;
            add(conn, path, library_id, album, settings)?
        }
    };
    set_enabled(conn, id, enabled)?;
    Ok(id)
}

/// Retire le dossier de la surveillance ; il n'est pas touché.
pub fn delete(conn: &Connection, id: i64) -> Result<(), ImportError> {
    if conn.execute("DELETE FROM watched_folders WHERE id = ?1", [id])? == 0 {
        return Err(ImportError::NotFound(id));
    }
    Ok(())
}

fn hidden(path: &Path) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

/// Fichier média absent du relevé fait à l'activation.
pub fn arrived(folder: &WatchedFolder, path: &Path) -> bool {
    if hidden(path) || format_of(path).is_none() || !path.is_file() {
        return false;
    }
    folder
        .relative(path)
        .is_some_and(|rel| !folder.existing.contains(&rel))
}

/// Médias arrivés dans le dossier depuis l'activation (relecture complète).
pub fn arrivals(folder: &WatchedFolder) -> Vec<PathBuf> {
    WalkDir::new(&folder.path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !hidden(e.path()))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|p| arrived(folder, p))
        .collect()
}

/// Fichiers en cours d'écriture : un fichier est prêt quand sa taille n'a
/// pas bougé pendant `STABLE`.
#[derive(Debug, Default)]
pub struct Tracker {
    /// Taille vue et moment où elle a été vue pour la première fois.
    pending: HashMap<PathBuf, (u64, Instant)>,
    /// Déjà rendus prêts : on ne les reprend pas à la relecture suivante.
    done: HashSet<PathBuf>,
}

impl Tracker {
    pub fn note(&mut self, path: PathBuf, now: Instant) {
        if self.done.contains(&path) || self.pending.contains_key(&path) {
            return;
        }
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        self.pending.insert(path, (size, now));
    }

    /// Fichiers complets, retirés de l'attente ; un fichier disparu est oublié.
    pub fn ready(&mut self, now: Instant) -> Vec<PathBuf> {
        let mut out = Vec::new();
        self.pending.retain(|path, (size, seen)| {
            let Ok(meta) = std::fs::metadata(path) else {
                return false;
            };
            if meta.len() != *size {
                *size = meta.len();
                *seen = now;
                return true;
            }
            // Ouvrable en lecture : l'écrivain l'a relâché.
            if meta.len() > 0
                && now.duration_since(*seen) >= STABLE
                && std::fs::File::open(path.as_path()).is_ok()
            {
                out.push(path.clone());
                return false;
            }
            true
        });
        self.done.extend(out.iter().cloned());
        out
    }

    /// Un fichier à reprendre plus tard (import déjà en cours).
    pub fn retry(&mut self, path: PathBuf, now: Instant) {
        self.done.remove(&path);
        self.note(path, now);
    }

    pub fn waiting(&self) -> usize {
        self.pending.len()
    }
}

/// Photos importées depuis un dossier surveillé.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchImport {
    pub folder_id: i64,
    pub library_id: i64,
    pub import_id: i64,
    pub media_ids: Vec<i64>,
}

/// Importe des fichiers complets du dossier dans son album ; ceux déjà dans
/// la bibliothèque sont ignorés. `None` s'il ne reste rien à importer.
pub fn import_files(
    conn: &mut Connection,
    folder: &WatchedFolder,
    files: Vec<PathBuf>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&ImportProgress),
) -> Result<Option<WatchImport>, ImportError> {
    let known = known_fingerprints(conn, folder.library_id)?;
    let fresh: Vec<PathBuf> = files
        .into_iter()
        .filter(|p| {
            format_of(p)
                .and_then(|f| scan::describe(p, f).ok())
                .is_some_and(|d| !known.contains(&d.fingerprint))
        })
        .collect();
    if fresh.is_empty() {
        return Ok(None);
    }
    let s = &folder.settings;
    let label = folder
        .path
        .file_name()
        .map_or_else(|| folder.path.to_string_lossy(), |n| n.to_string_lossy())
        .into_owned();
    let options = ImportOptions {
        library_id: folder.library_id,
        source_path: folder.path.clone(),
        source_label: label,
        files: fresh,
        album_name: folder.album_name.clone(),
        album_per_day: false,
        rename: s.rename,
        backup: s.backup,
        keywords: s.keywords.clone(),
        creator: None,
        rights: s.rights.clone(),
        skip_pair_jpegs: s.skip_pair_jpegs,
        client_id: None,
        session_type_id: None,
        session_label: String::new(),
    };
    let import_id = super::plan(conn, &options)?;
    let mut media_ids = Vec::new();
    super::run(conn, import_id, cancel, |p| {
        if let Some(id) = p.media_id {
            media_ids.push(id);
        }
        progress(p);
    })?;
    Ok(Some(WatchImport {
        folder_id: folder.id,
        library_id: folder.library_id,
        import_id,
        media_ids,
    }))
}

/// Supprime ceux des `files` qui sont déjà dans la bibliothèque ; rend le
/// nombre de fichiers supprimés. Réservé aux copies de réception, jamais aux
/// originaux.
pub fn remove_imported(
    conn: &Connection,
    library_id: i64,
    files: &[PathBuf],
) -> Result<usize, ImportError> {
    let known = known_fingerprints(conn, library_id)?;
    Ok(files
        .iter()
        .filter(|p| {
            format_of(p)
                .and_then(|f| scan::describe(p, f).ok())
                .is_some_and(|d| known.contains(&d.fingerprint))
        })
        .filter(|p| std::fs::remove_file(p).is_ok())
        .count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn fichier_pret_quand_sa_taille_ne_bouge_plus() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DSCF0001.RAF");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&[1; 1000]).unwrap();
        let start = Instant::now();
        let mut tracker = Tracker::default();
        tracker.note(path.clone(), start);
        assert!(tracker.ready(start + STABLE / 2).is_empty());
        // Le logiciel du boîtier écrit encore : l'attente repart.
        file.write_all(&[2; 1000]).unwrap();
        file.flush().unwrap();
        assert!(
            tracker
                .ready(start + STABLE + Duration::from_millis(10))
                .is_empty()
        );
        drop(file);
        let later = start + STABLE * 3;
        assert_eq!(tracker.ready(later), vec![path.clone()]);
        // Déjà rendu : une relecture ne le reprend pas.
        tracker.note(path.clone(), later);
        assert_eq!(tracker.waiting(), 0);
        // Un fichier vide n'est jamais prêt.
        let empty = dir.path().join("vide.jpg");
        std::fs::File::create(&empty).unwrap();
        tracker.note(empty, later);
        assert!(tracker.ready(later + STABLE * 2).is_empty());
    }
}
