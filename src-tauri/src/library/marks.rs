//! Notes, drapeaux et étiquettes de couleur : écrits en base tout de suite,
//! puis dans le XMP de chaque photo, de façon groupée et en arrière-plan
//! (cahier des charges, section 5).
//!
//! Chaque modification d'une photo (marque, retouche) incrémente
//! `xmp_pending`. [`flush_pending_xmp`] relit son XMP, met à jour la note, le
//! drapeau, la couleur et les retouches sans toucher au reste, puis la retire
//! de la file seulement si le compteur n'a pas bougé pendant l'écriture : une
//! modification concurrente est réécrite au passage suivant.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use rusqlite::{Connection, params};
use serde::Serialize;

use super::{Library, LibraryError, get_library, paths};
use crate::db::now_utc;
use crate::metadata::xmp::{self, ColorLabel, Flag, LocationSource, XmpLocation};

#[derive(Debug, thiserror::Error)]
pub enum MarkError {
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("note hors limites : {0}")]
    InvalidRating(u8),
    #[error(transparent)]
    Develop(#[from] crate::develop::store::StoreError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Rating(u8),
    Flag(Flag),
    Color(Option<ColorLabel>),
}

/// Applique une marque à des photos ; renvoie le nombre de photos modifiées.
pub fn apply_mark(conn: &mut Connection, ids: &[i64], mark: Mark) -> Result<usize, MarkError> {
    let now = now_utc();
    let (column, value): (&str, rusqlite::types::Value) = match mark {
        Mark::Rating(r) if r > 5 => return Err(MarkError::InvalidRating(r)),
        Mark::Rating(r) => ("rating", i64::from(r).into()),
        Mark::Flag(f) => ("flag", f.as_db().into()),
        Mark::Color(c) => (
            "color",
            c.map_or(rusqlite::types::Value::Null, |c| {
                c.as_db().to_owned().into()
            }),
        ),
    };
    let sql = format!(
        "UPDATE media SET {column} = ?2, xmp_pending = xmp_pending + 1, updated_at = ?3
         WHERE id = ?1 AND {column} IS NOT ?2"
    );
    // Les marques appartiennent au déclenchement : toute la pile RAW + JPEG.
    let ids = super::pairs::expand(conn, ids, None)?;
    let tx = conn.transaction()?;
    let mut changed = 0;
    {
        let mut stmt = tx.prepare(&sql)?;
        for id in &ids {
            changed += stmt.execute(params![id, value, now])?;
        }
    }
    tx.commit()?;
    Ok(changed)
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlushReport {
    pub written: usize,
    /// Bibliothèque débranchée : l'écriture attend son retour.
    pub offline: usize,
    pub failed: usize,
}

struct Pending {
    id: i64,
    library_id: i64,
    sidecar: Option<String>,
    rating: i64,
    flag: i64,
    color: Option<String>,
    /// Compteur de modifications lu avec la photo.
    revision: i64,
    /// Lieu de la base.
    location: Option<XmpLocation>,
}

/// Écrit dans les XMP jusqu'à `limit` photos en attente.
pub fn flush_pending_xmp(conn: &Connection, limit: usize) -> Result<FlushReport, MarkError> {
    let pending: Vec<Pending> = conn
        .prepare(
            "SELECT id, library_id, sidecar_path, rating, flag, color, xmp_pending,
                    lat, lon, location_source FROM media
             WHERE xmp_pending > 0 AND state = 'normal'
             ORDER BY updated_at, id LIMIT ?1",
        )?
        .query_map([limit as i64], |r| {
            Ok(Pending {
                id: r.get(0)?,
                library_id: r.get(1)?,
                sidecar: r.get(2)?,
                rating: r.get(3)?,
                flag: r.get(4)?,
                color: r.get(5)?,
                revision: r.get(6)?,
                location: match (r.get(7)?, r.get(8)?, r.get::<_, Option<String>>(9)?) {
                    (Some(latitude), Some(longitude), Some(source)) => {
                        LocationSource::from_db(&source).map(|source| XmpLocation {
                            latitude,
                            longitude,
                            source,
                        })
                    }
                    _ => None,
                },
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut libraries: HashMap<i64, Library> = HashMap::new();
    let mut report = FlushReport::default();
    for p in pending {
        let lib = match libraries.entry(p.library_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(get_library(conn, p.library_id)?),
        };
        if !lib.is_online {
            report.offline += 1;
            continue;
        }
        // Les retouches partent dans le même XMP que les marques.
        let develop = crate::develop::store::xmp_data(conn, p.id)?;
        let result = match &p.sidecar {
            Some(sidecar) => write_marks(lib, sidecar, &p, develop),
            // Pas d'annexe connue : rien à écrire.
            None => Ok(()),
        };
        match result {
            Ok(()) => {
                // Retirée de la file seulement si rien n'a changé pendant l'écriture.
                conn.execute(
                    "UPDATE media SET xmp_pending = 0 WHERE id = ?1 AND xmp_pending = ?2",
                    params![p.id, p.revision],
                )?;
                report.written += 1;
            }
            Err(error) => {
                eprintln!("XMP du média {} : {error}", p.id);
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

fn write_marks(
    lib: &Library,
    sidecar: &str,
    p: &Pending,
    develop: crate::develop::store::DevelopXmp,
) -> Result<(), String> {
    let path = paths::to_absolute(&lib.root_path, sidecar).map_err(|e| e.to_string())?;
    let mut data = xmp::read_sidecar(&path)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    data.rating = u8::try_from(p.rating.clamp(0, 5)).unwrap_or(0);
    data.flag = Flag::from_db(p.flag);
    data.label = p.color.as_deref().and_then(ColorLabel::from_db);
    data.develop = develop.develop;
    data.snapshots = develop.snapshots;
    data.versions = develop.versions;
    // Le GPS du boîtier reste dans l'original ; seul un lieu posé après coup
    // va dans le XMP.
    if let Some(loc) = p.location.filter(|l| l.source != LocationSource::Exif) {
        data.location = Some(loc);
    }
    xmp::write_sidecar(&path, &data).map_err(|e| e.to_string())
}

/// Photos encore en attente d'écriture XMP.
pub fn pending_count(conn: &Connection) -> Result<i64, MarkError> {
    Ok(conn.query_row(
        "SELECT count(*) FROM media WHERE xmp_pending > 0 AND state = 'normal'",
        [],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("bib");
        std::fs::create_dir_all(root.join("2026")).unwrap();
        let conn = db::open_in_memory().unwrap();
        let lib = crate::library::create_library(&conn, "Photos", &root, None).unwrap();
        let album = crate::library::find_or_create_album(&conn, lib.id, "A", "2026", None).unwrap();
        conn.execute(
            "INSERT INTO media (library_id, album_id, kind, path, sidecar_path, original_name,
                                file_size, fingerprint, content_hash, imported_at, updated_at)
             VALUES (?1, ?2, 'photo', '2026/a.RAF', '2026/a.xmp', 'a.RAF', 1, 'f', 'h', 'x', 'x')",
            params![lib.id, album],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        (dir, conn, id)
    }

    fn sidecar(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("bib").join("2026").join("a.xmp")
    }

    #[test]
    fn note_drapeau_couleur_en_base_puis_dans_le_xmp() {
        let (dir, mut conn, id) = setup();
        assert_eq!(apply_mark(&mut conn, &[id], Mark::Rating(4)).unwrap(), 1);
        apply_mark(&mut conn, &[id], Mark::Flag(Flag::Picked)).unwrap();
        apply_mark(&mut conn, &[id], Mark::Color(Some(ColorLabel::Green))).unwrap();
        assert_eq!(pending_count(&conn).unwrap(), 1);

        let report = flush_pending_xmp(&conn, 100).unwrap();
        assert_eq!(report.written, 1);
        assert_eq!(pending_count(&conn).unwrap(), 0);
        let data = xmp::read_sidecar(&sidecar(&dir)).unwrap().unwrap();
        assert_eq!(
            (data.rating, data.flag, data.label),
            (4, Flag::Picked, Some(ColorLabel::Green))
        );
    }

    #[test]
    fn la_meme_valeur_ne_relance_pas_d_ecriture() {
        let (_dir, mut conn, id) = setup();
        assert_eq!(apply_mark(&mut conn, &[id], Mark::Rating(0)).unwrap(), 0);
        assert_eq!(pending_count(&conn).unwrap(), 0);
    }

    #[test]
    fn refuse_une_note_hors_limites() {
        let (_dir, mut conn, id) = setup();
        assert!(matches!(
            apply_mark(&mut conn, &[id], Mark::Rating(6)),
            Err(MarkError::InvalidRating(6))
        ));
    }

    #[test]
    fn conserve_les_proprietes_des_autres_logiciels() {
        let (dir, mut conn, id) = setup();
        std::fs::write(
            sidecar(&dir),
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/" crs:Exposure2012="+0.50">
   <dc:subject><rdf:Bag><rdf:li>mer</rdf:li></rdf:Bag></dc:subject>
  </rdf:Description></rdf:RDF></x:xmpmeta>"#,
        )
        .unwrap();
        apply_mark(&mut conn, &[id], Mark::Rating(5)).unwrap();
        flush_pending_xmp(&conn, 100).unwrap();
        let text = std::fs::read_to_string(sidecar(&dir)).unwrap();
        assert!(text.contains("Exposure2012"), "{text}");
        let data = xmp::read_sidecar(&sidecar(&dir)).unwrap().unwrap();
        assert_eq!(
            (data.rating, data.keywords.as_slice()),
            (5, ["mer".to_owned()].as_slice())
        );
    }

    #[test]
    fn modification_pendant_l_ecriture_reste_en_attente() {
        let (_dir, mut conn, id) = setup();
        apply_mark(&mut conn, &[id], Mark::Rating(2)).unwrap();
        // Simule une note changée entre la lecture de la file et la fin de l'écriture :
        // la condition de retrait ne correspond plus.
        let read: i64 = conn
            .query_row("SELECT xmp_pending FROM media WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        apply_mark(&mut conn, &[id], Mark::Rating(3)).unwrap();
        conn.execute(
            "UPDATE media SET xmp_pending = 0 WHERE id = ?1 AND xmp_pending = ?2",
            params![id, read],
        )
        .unwrap();
        assert_eq!(pending_count(&conn).unwrap(), 1);
    }

    #[test]
    fn bibliotheque_hors_ligne_attend_son_retour() {
        let (dir, mut conn, id) = setup();
        apply_mark(&mut conn, &[id], Mark::Rating(3)).unwrap();
        let root = dir.path().join("bib");
        let moved = dir.path().join("debranchee");
        std::fs::rename(&root, &moved).unwrap();
        let report = flush_pending_xmp(&conn, 100).unwrap();
        assert_eq!((report.written, report.offline), (0, 1));
        assert_eq!(pending_count(&conn).unwrap(), 1);

        std::fs::rename(&moved, &root).unwrap();
        assert_eq!(flush_pending_xmp(&conn, 100).unwrap().written, 1);
        assert_eq!(pending_count(&conn).unwrap(), 0);
    }
}
