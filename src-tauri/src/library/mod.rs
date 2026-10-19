//! Bibliothèques, albums et matériel détecté.

pub mod cloud;
pub mod filter;
pub mod identity;
pub mod marks;
pub mod media;
pub mod pairs;
pub mod paths;
pub mod template;
pub mod trash;

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::db::now_utc;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("erreur SQLite : {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("bibliothèque {0} introuvable")]
    NotFound(i64),
    #[error("le dossier {0} n'existe pas ou n'est pas accessible")]
    MissingRoot(PathBuf),
    #[error(transparent)]
    Template(#[from] template::TemplateError),
    #[error("la photo {media_id} n'appartient pas à l'album {album_id}")]
    NotInAlbum { album_id: i64, media_id: i64 },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub root_path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub folder_template: String,
    pub rename_template: Option<String>,
    pub is_online: bool,
}

pub fn create_library(
    conn: &Connection,
    name: &str,
    root: &Path,
    backup: Option<&Path>,
) -> Result<Library, LibraryError> {
    if !root.is_dir() {
        return Err(LibraryError::MissingRoot(root.to_path_buf()));
    }
    conn.execute(
        "INSERT INTO libraries (name, root_path, backup_path, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            paths::nfc(name),
            root.to_string_lossy(),
            backup.map(|b| b.to_string_lossy().into_owned()),
            now_utc()
        ],
    )?;
    get_library(conn, conn.last_insert_rowid())
}

pub fn get_library(conn: &Connection, id: i64) -> Result<Library, LibraryError> {
    conn.query_row(
        "SELECT id, name, root_path, backup_path, folder_template, rename_template
         FROM libraries WHERE id = ?1",
        [id],
        |r| {
            let root: String = r.get(2)?;
            Ok(Library {
                id: r.get(0)?,
                name: r.get(1)?,
                is_online: Path::new(&root).is_dir(),
                root_path: root.into(),
                backup_path: r.get::<_, Option<String>>(3)?.map(PathBuf::from),
                folder_template: r.get(4)?,
                rename_template: r.get(5)?,
            })
        },
    )
    .optional()?
    .ok_or(LibraryError::NotFound(id))
}

pub fn list_libraries(conn: &Connection) -> Result<Vec<Library>, LibraryError> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM libraries ORDER BY name")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    ids.into_iter().map(|id| get_library(conn, id)).collect()
}

pub fn set_templates(
    conn: &Connection,
    id: i64,
    folder_template: &str,
    rename_template: Option<&str>,
) -> Result<(), LibraryError> {
    template::validate(folder_template)?;
    if let Some(rename) = rename_template {
        template::validate(rename)?;
    }
    conn.execute(
        "UPDATE libraries SET folder_template = ?2, rename_template = ?3 WHERE id = ?1",
        params![id, folder_template, rename_template],
    )?;
    Ok(())
}

/// Note la présence du disque d'une bibliothèque : `is_online`, et la date
/// du dernier passage quand il est branché.
pub fn record_presence(conn: &Connection, id: i64, online: bool) -> Result<(), LibraryError> {
    conn.execute(
        "UPDATE libraries SET is_online = ?2,
                last_seen_at = CASE WHEN ?2 THEN ?3 ELSE last_seen_at END
         WHERE id = ?1",
        params![id, online, now_utc()],
    )?;
    Ok(())
}

/// Dernière fois que le disque de la bibliothèque a été vu branché.
pub fn last_seen(conn: &Connection, id: i64) -> Result<Option<String>, LibraryError> {
    Ok(conn
        .query_row(
            "SELECT last_seen_at FROM libraries WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

/// Photos dont les marques attendent d'être écrites dans leur XMP.
pub fn pending_xmp(conn: &Connection, id: i64) -> Result<i64, LibraryError> {
    Ok(conn.query_row(
        "SELECT count(*) FROM media WHERE library_id = ?1 AND xmp_pending > 0 AND state = 'normal'",
        [id],
        |r| r.get(0),
    )?)
}

/// Dossier de sauvegarde (absolu), ou aucun.
pub fn set_backup(conn: &Connection, id: i64, backup: Option<&Path>) -> Result<(), LibraryError> {
    if let Some(b) = backup
        && !b.is_dir()
    {
        return Err(LibraryError::MissingRoot(b.to_path_buf()));
    }
    let changed = conn.execute(
        "UPDATE libraries SET backup_path = ?2 WHERE id = ?1",
        params![id, backup.map(|b| b.to_string_lossy().into_owned())],
    )?;
    if changed == 0 {
        return Err(LibraryError::NotFound(id));
    }
    Ok(())
}

/// Album du dossier `folder` (relatif), créé s'il n'existe pas.
/// Les dates de l'album s'étendent pour couvrir `date` (`AAAA-MM-JJ`).
pub fn find_or_create_album(
    conn: &Connection,
    library_id: i64,
    name: &str,
    folder: &str,
    date: Option<&str>,
) -> Result<i64, LibraryError> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM albums WHERE library_id = ?1 AND folder = ?2",
            params![library_id, folder],
            |r| r.get(0),
        )
        .optional()?;
    let id = match existing {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO albums (library_id, name, folder, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![library_id, paths::nfc(name), folder, now_utc()],
            )?;
            conn.last_insert_rowid()
        }
    };
    if let Some(date) = date {
        conn.execute(
            "UPDATE albums SET
               start_date = CASE WHEN start_date IS NULL OR ?2 < start_date THEN ?2 ELSE start_date END,
               end_date   = CASE WHEN end_date   IS NULL OR ?2 > end_date   THEN ?2 ELSE end_date   END
             WHERE id = ?1",
            params![id, date],
        )?;
    }
    Ok(id)
}

/// Choisit la photo de couverture d'un album ; la photo doit en faire partie.
pub fn set_album_cover(
    conn: &Connection,
    album_id: i64,
    media_id: i64,
) -> Result<(), LibraryError> {
    let changed = conn.execute(
        "UPDATE albums SET cover_media_id = ?2
         WHERE id = ?1 AND EXISTS (SELECT 1 FROM media WHERE id = ?2 AND album_id = ?1)",
        params![album_id, media_id],
    )?;
    if changed == 0 {
        return Err(LibraryError::NotInAlbum { album_id, media_id });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GearKind {
    Body,
    Lens,
}

impl GearKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Lens => "lens",
        }
    }
}

/// Fiche de matériel correspondant aux chaînes EXIF, créée au premier passage.
pub fn find_or_create_gear(
    conn: &Connection,
    kind: GearKind,
    make: &str,
    model: &str,
    serial: Option<&str>,
) -> Result<i64, LibraryError> {
    let serial = serial.unwrap_or_default();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT a.gear_id FROM gear_aliases a JOIN gear g ON g.id = a.gear_id
             WHERE g.kind = ?1 AND a.exif_make = ?2 AND a.exif_model = ?3 AND a.serial = ?4",
            params![kind.as_str(), make, model, serial],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO gear (kind, make, model, serial, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            kind.as_str(),
            make,
            model,
            (!serial.is_empty()).then_some(serial),
            now_utc()
        ],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO gear_aliases (gear_id, exif_make, exif_model, serial) VALUES (?1, ?2, ?3, ?4)",
        params![id, make, model, serial],
    )?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn bibliotheque_et_albums() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let lib = create_library(&conn, "Photos", dir.path(), None).unwrap();
        assert!(lib.is_online);
        assert_eq!(lib.folder_template, template::DEFAULT_FOLDER_TEMPLATE);

        let a = find_or_create_album(
            &conn,
            lib.id,
            "Sortie",
            "2026/2026-10-18 Sortie",
            Some("2026-10-18"),
        )
        .unwrap();
        let b = find_or_create_album(
            &conn,
            lib.id,
            "Sortie",
            "2026/2026-10-18 Sortie",
            Some("2026-10-19"),
        )
        .unwrap();
        assert_eq!(a, b);
        let (start, end): (String, String) = conn
            .query_row(
                "SELECT start_date, end_date FROM albums WHERE id = ?1",
                [a],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((start.as_str(), end.as_str()), ("2026-10-18", "2026-10-19"));
    }

    #[test]
    fn materiel_reconnu_par_son_numero_de_serie() {
        let conn = db::open_in_memory().unwrap();
        let a = find_or_create_gear(&conn, GearKind::Body, "Fujifilm", "X-T3", Some("1")).unwrap();
        let b = find_or_create_gear(&conn, GearKind::Body, "Fujifilm", "X-T3", Some("1")).unwrap();
        let c = find_or_create_gear(&conn, GearKind::Body, "Fujifilm", "X-T3", Some("2")).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn refuse_un_modele_invalide() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let lib = create_library(&conn, "Photos", dir.path(), None).unwrap();
        assert!(set_templates(&conn, lib.id, "{inconnu}", None).is_err());
    }
}
