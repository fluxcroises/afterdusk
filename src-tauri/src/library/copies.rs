//! Copies virtuelles dans les grilles : chacune apparaît juste après sa photo,
//! avec son propre aperçu rendu, sous un identifiant négatif (moins
//! l'identifiant de sa version dans `edits`). Les marques (note, drapeau,
//! couleur) restent celles de la photo : les commandes qui reçoivent un
//! identifiant négatif l'appliquent à la photo.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::raw::preview::{SIZES, cache_path};

/// Une copie virtuelle : identifiant de grille (négatif), photo, version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copy {
    pub item_id: i64,
    pub media_id: i64,
    pub version: String,
}

pub fn is_copy(id: i64) -> bool {
    id < 0
}

/// Photo et version d'un identifiant de grille ; `None` : copie disparue.
/// Une photo (identifiant positif) est sa version originale.
pub fn resolve(conn: &Connection, id: i64) -> rusqlite::Result<Option<(i64, String)>> {
    if !is_copy(id) {
        return Ok(Some((id, String::new())));
    }
    conn.query_row(
        "SELECT media_id, name FROM edits WHERE id = ?1 AND name <> ''",
        [-id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// Les photos désignées : une copie compte pour sa photo ; sans doublon,
/// dans l'ordre.
pub fn media_ids(conn: &Connection, ids: &[i64]) -> rusqlite::Result<Vec<i64>> {
    let mut out = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some((media, _)) = resolve(conn, id)?
            && !out.contains(&media)
        {
            out.push(media);
        }
    }
    Ok(out)
}

/// Copies des photos d'une bibliothèque, par photo, dans l'ordre de création.
pub fn copies_by_media(
    conn: &Connection,
    library_id: i64,
) -> rusqlite::Result<HashMap<i64, Vec<Copy>>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.media_id, e.name FROM edits e JOIN media m ON m.id = e.media_id
         WHERE m.library_id = ?1 AND e.name <> '' ORDER BY e.created_at, e.id",
    )?;
    let mut out: HashMap<i64, Vec<Copy>> = HashMap::new();
    for row in stmt.query_map([library_id], |r| {
        Ok(Copy {
            item_id: -r.get::<_, i64>(0)?,
            media_id: r.get(1)?,
            version: r.get(2)?,
        })
    })? {
        let copy = row?;
        out.entry(copy.media_id).or_default().push(copy);
    }
    Ok(out)
}

/// Les trois paliers d'aperçu d'une copie sont-ils en cache ?
pub fn has_previews(cache_root: &Path, item_id: i64) -> bool {
    SIZES
        .iter()
        .all(|&s| cache_path(cache_root, item_id, s).is_file())
}

/// Copies (toutes, ou celles d'une photo) dont la retouche est plus récente
/// que les aperçus, ou sans aperçu.
pub fn stale_copies(
    conn: &Connection,
    cache_root: &Path,
    media_id: Option<i64>,
) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.updated_at FROM edits e JOIN media m ON m.id = e.media_id
         WHERE e.name <> '' AND m.state = 'normal' AND (?1 IS NULL OR e.media_id = ?1)",
    )?;
    let rows: Vec<(i64, String)> = stmt
        .query_map(params![media_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .filter(|(edit, updated)| {
            let item = -edit;
            let made = std::fs::metadata(cache_path(cache_root, item, SIZES[0]))
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    chrono::DateTime::<chrono::Utc>::from(t)
                        .format("%Y-%m-%dT%H:%M:%SZ")
                        .to_string()
                });
            made.is_none_or(|made| *updated >= made)
        })
        .map(|(edit, _)| -edit)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn identifiants_de_copies() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(resolve(&conn, 7).unwrap(), Some((7, String::new())));
        assert_eq!(resolve(&conn, -99).unwrap(), None);
        assert!(is_copy(-3) && !is_copy(3));
    }
}
