//! Collections (cahier des charges, 4.2) : regroupements de photos de
//! plusieurs albums, sans rien déplacer. Une collection manuelle garde la
//! liste des photos qu'on y ajoute ; un album intelligent garde un filtre et
//! se recalcule à chaque ouverture. Elles vivent dans la base seulement,
//! comme les collections de Lightroom : une base perdue puis reconstruite
//! depuis les XMP ne les retrouve pas.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::LibraryError;
use super::filter::MediaFilter;
use super::pairs;
use crate::db::now_utc;

/// Version du filtre enregistré ; toute évolution du format prévoit une
/// migration dans `decode`.
pub const FILTER_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CollectionKind {
    Manual,
    Smart,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub kind: CollectionKind,
    /// Albums intelligents seulement.
    pub filter: Option<MediaFilter>,
    /// Photos visibles (hors corbeille, une par pile).
    pub count: i64,
}

#[derive(Serialize, Deserialize)]
struct StoredFilter {
    version: u32,
    filter: MediaFilter,
}

fn encode(filter: &MediaFilter) -> String {
    // Un album intelligent ne dépend pas d'une collection manuelle, qui
    // pourrait disparaître.
    let filter = MediaFilter {
        collection_id: None,
        ..filter.clone()
    };
    serde_json::to_string(&StoredFilter {
        version: FILTER_VERSION,
        filter,
    })
    .unwrap_or_else(|_| "{}".to_owned())
}

/// Filtre enregistré ; un texte illisible donne un filtre vide plutôt
/// qu'une erreur, pour que la liste des collections reste affichable.
fn decode(json: &str) -> MediaFilter {
    match serde_json::from_str::<StoredFilter>(json) {
        // Version 1 : format actuel. Les champs inconnus ou absents prennent
        // leur valeur par défaut (`serde(default)`).
        Ok(stored) if stored.version <= FILTER_VERSION => stored.filter,
        _ => MediaFilter::default(),
    }
}

fn clean_name(name: &str) -> Result<String, LibraryError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LibraryError::EmptyName);
    }
    Ok(name.to_owned())
}

/// Photos visibles d'une grille filtrée, sans les lister.
pub fn count(
    conn: &Connection,
    library_id: i64,
    filter: &MediaFilter,
) -> Result<i64, LibraryError> {
    let (mut clauses, mut values) = filter.to_sql();
    clauses.insert(0, "m.library_id = ?".to_owned());
    values.insert(0, library_id.into());
    clauses.push("m.state = 'normal'".to_owned());
    clauses.push(pairs::HEAD_SQL.to_owned());
    Ok(conn.query_row(
        &format!(
            "SELECT count(*) FROM media m WHERE {}",
            clauses.join(" AND ")
        ),
        rusqlite::params_from_iter(values),
        |r| r.get(0),
    )?)
}

/// Collections d'une bibliothèque, par nom.
pub fn list(conn: &Connection, library_id: i64) -> Result<Vec<Collection>, LibraryError> {
    let rows: Vec<(i64, String, String, Option<String>)> = conn
        .prepare(
            "SELECT id, name, kind, filter_json FROM collections
              WHERE library_id = ?1 ORDER BY name COLLATE NOCASE, id",
        )?
        .query_map([library_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    rows.into_iter()
        .map(|(id, name, kind, json)| {
            let (kind, filter) = if kind == "smart" {
                (CollectionKind::Smart, decode(json.as_deref().unwrap_or("")))
            } else {
                (
                    CollectionKind::Manual,
                    MediaFilter {
                        collection_id: Some(id),
                        ..Default::default()
                    },
                )
            };
            let count = count(conn, library_id, &filter)?;
            Ok(Collection {
                id,
                name,
                kind,
                filter: (kind == CollectionKind::Smart).then_some(filter),
                count,
            })
        })
        .collect()
}

fn kind_of(conn: &Connection, id: i64) -> Result<CollectionKind, LibraryError> {
    let kind: String = conn
        .query_row("SELECT kind FROM collections WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or(LibraryError::CollectionNotFound(id))?;
    Ok(if kind == "smart" {
        CollectionKind::Smart
    } else {
        CollectionKind::Manual
    })
}

/// Nouvelle collection : manuelle sans filtre, intelligente avec.
pub fn create(
    conn: &Connection,
    library_id: i64,
    name: &str,
    filter: Option<&MediaFilter>,
) -> Result<i64, LibraryError> {
    let name = clean_name(name)?;
    conn.execute(
        "INSERT INTO collections (library_id, name, kind, filter_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            library_id,
            name,
            if filter.is_some() { "smart" } else { "manual" },
            filter.map(encode),
            now_utc()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<(), LibraryError> {
    let name = clean_name(name)?;
    if conn.execute(
        "UPDATE collections SET name = ?2 WHERE id = ?1",
        params![id, name],
    )? == 0
    {
        return Err(LibraryError::CollectionNotFound(id));
    }
    Ok(())
}

/// Remplace le filtre d'un album intelligent.
pub fn set_filter(conn: &Connection, id: i64, filter: &MediaFilter) -> Result<(), LibraryError> {
    if kind_of(conn, id)? != CollectionKind::Smart {
        return Err(LibraryError::NotSmart(id));
    }
    conn.execute(
        "UPDATE collections SET filter_json = ?2 WHERE id = ?1",
        params![id, encode(filter)],
    )?;
    Ok(())
}

/// Supprime la collection ; ses photos restent dans leurs albums.
pub fn delete(conn: &Connection, id: i64) -> Result<(), LibraryError> {
    if conn.execute("DELETE FROM collections WHERE id = ?1", [id])? == 0 {
        return Err(LibraryError::CollectionNotFound(id));
    }
    Ok(())
}

/// Ajoute des photos à la fin d'une collection manuelle ; celles qui y sont
/// déjà gardent leur place. Renvoie le nombre de photos ajoutées.
pub fn add_media(conn: &mut Connection, id: i64, media_ids: &[i64]) -> Result<usize, LibraryError> {
    if kind_of(conn, id)? != CollectionKind::Manual {
        return Err(LibraryError::NotManual(id));
    }
    let tx = conn.transaction()?;
    let mut position: i64 = tx.query_row(
        "SELECT coalesce(max(position), -1) FROM collection_media WHERE collection_id = ?1",
        [id],
        |r| r.get(0),
    )?;
    let mut added = 0;
    for media in media_ids {
        let exists: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM media WHERE id = ?1)",
            [media],
            |r| r.get(0),
        )?;
        if !exists {
            continue;
        }
        position += 1;
        added += tx.execute(
            "INSERT OR IGNORE INTO collection_media (collection_id, media_id, position)
             VALUES (?1, ?2, ?3)",
            params![id, media, position],
        )?;
    }
    tx.commit()?;
    Ok(added)
}

/// Retire des photos d'une collection manuelle ; elles restent dans leurs albums.
pub fn remove_media(conn: &Connection, id: i64, media_ids: &[i64]) -> Result<usize, LibraryError> {
    if kind_of(conn, id)? != CollectionKind::Manual {
        return Err(LibraryError::NotManual(id));
    }
    let mut removed = 0;
    for media in media_ids {
        removed += conn.execute(
            "DELETE FROM collection_media WHERE collection_id = ?1 AND media_id = ?2",
            params![id, media],
        )?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::filter::{RatingFilter, RatingOp};

    #[test]
    fn filtre_enregistre_et_relu() {
        let f = MediaFilter {
            rating: Some(RatingFilter {
                op: RatingOp::Exactly,
                value: 5,
            }),
            focal: Some([23.0, 23.0]),
            collection_id: Some(4),
            ..Default::default()
        };
        let json = encode(&f);
        assert!(json.starts_with("{\"version\":1,"), "{json}");
        let back = decode(&json);
        assert_eq!(back.rating, f.rating);
        assert_eq!(back.focal, f.focal);
        assert_eq!(
            back.collection_id, None,
            "jamais lié à une collection manuelle"
        );
        // Champ absent (filtre écrit avant l'ajout de la focale) : valeur par défaut.
        let old = decode(r#"{"version":1,"filter":{"flags":[1]}}"#);
        assert_eq!((old.flags, old.focal), (vec![1], None));
        // Version future ou texte abîmé : filtre vide, la liste reste lisible.
        assert!(decode(r#"{"version":99,"filter":{"flags":[1]}}"#).is_empty());
        assert!(decode("pas du json").is_empty());
    }
}
