//! Lieu nommé des photos localisées (ville, région, pays, fuseau) et sa
//! place dans la recherche : « Cabourg » ou « France » retrouvent la photo.

use rusqlite::{Connection, OptionalExtension, params};

use super::places;

/// Ce qu'une photo met dans la recherche.
struct SearchRow {
    album: String,
    file: String,
    original: String,
    keywords: String,
    city: Option<String>,
    region: Option<String>,
    country: Option<String>,
}

/// Reconstruit la ligne de recherche d'une photo, lieu compris (table FTS5
/// sans contenu : la ligne est retirée puis réinsérée).
pub fn reindex(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    let row = conn
        .query_row(
            "SELECT a.name, replace(m.path, rtrim(m.path, replace(m.path, '/', '')), ''),
                    m.original_name,
                    coalesce((SELECT group_concat(k.word, ' ')
                                FROM media_keywords mk JOIN keywords k ON k.id = mk.keyword_id
                               WHERE mk.media_id = m.id), ''),
                    m.city, m.region, m.country
               FROM media m JOIN albums a ON a.id = m.album_id
              WHERE m.id = ?1",
            [id],
            |r| {
                Ok(SearchRow {
                    album: r.get(0)?,
                    file: r.get(1)?,
                    original: r.get(2)?,
                    keywords: r.get(3)?,
                    city: r.get(4)?,
                    region: r.get(5)?,
                    country: r.get(6)?,
                })
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(());
    };
    let country_name = row.country.as_deref().and_then(places::country_name);
    let place = [
        row.city,
        row.region,
        country_name,
        row.country.filter(|c| !c.is_empty()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    conn.execute("DELETE FROM media_search WHERE rowid = ?1", [id])?;
    conn.execute(
        "INSERT INTO media_search (rowid, album, file_name, original_name, keywords, place)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, row.album, row.file, row.original, row.keywords, place],
    )?;
    Ok(())
}

/// Nomme le lieu d'une photo d'après ses coordonnées (ou l'efface si elle
/// n'en a pas), puis met sa recherche à jour.
pub fn name_media(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    let coords: Option<(Option<f64>, Option<f64>)> = conn
        .query_row("SELECT lat, lon FROM media WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    let named = match coords {
        Some((Some(lat), Some(lon))) => Some(places::nearest(lat, lon)),
        Some(_) => None,
        None => return Ok(()),
    };
    match named {
        Some(Some(p)) => conn.execute(
            "UPDATE media SET city = ?2, region = ?3, country = ?4, timezone = ?5 WHERE id = ?1",
            params![id, p.city, p.region, p.country, p.timezone],
        )?,
        // Localisée, mais loin de tout lieu habité : cherchée une fois.
        Some(None) => conn.execute(
            "UPDATE media SET city = NULL, region = NULL, country = '', timezone = NULL
             WHERE id = ?1",
            [id],
        )?,
        None => conn.execute(
            "UPDATE media SET city = NULL, region = NULL, country = NULL, timezone = NULL
             WHERE id = ?1",
            [id],
        )?,
    };
    reindex(conn, id)
}

/// Photos localisées sans lieu nommé (bases antérieures, GPS du boîtier) :
/// nomme jusqu'à `limit` d'entre elles, renvoie leur nombre.
pub fn name_pending(conn: &Connection, limit: usize) -> rusqlite::Result<usize> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM media WHERE lat IS NOT NULL AND country IS NULL LIMIT ?1")?
        .query_map([limit as i64], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for &id in &ids {
        name_media(conn, id)?;
    }
    Ok(ids.len())
}
