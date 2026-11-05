//! Piles RAW + JPEG des photos du boîtier. En prise de vue reliée, le X-H2
//! numérote les deux fichiers d'un déclenchement à la suite (`DSCF0001.raf`,
//! `DSCF0002.jpg`) au lieu de leur donner le même nom, et ils arrivent dans
//! deux imports séparés : l'appariement de l'import, par le nom, ne les
//! reconnaît pas. Ici, un RAW et l'image qui porte le numéro suivant, prise
//! au même moment dans le même album, forment une pile.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::library::{LibraryError, pairs};
use crate::metadata::media::{MediaFormat, format_of};

/// « DSCF0001.raf » → (« DSCF », 1, 4).
fn numbered(name: &str) -> Option<(&str, u64, usize)> {
    let stem = Path::new(name).file_stem()?.to_str()?;
    let digits = stem.len() - stem.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let (prefix, number) = stem.split_at(stem.len() - digits);
    Some((prefix, number.parse().ok()?, digits))
}

struct Shot {
    id: i64,
    album_id: i64,
    name: String,
    captured_at: Option<String>,
    raw: bool,
}

fn shot(conn: &Connection, id: i64) -> Result<Option<Shot>, LibraryError> {
    Ok(conn
        .query_row(
            "SELECT album_id, original_name, captured_at, path FROM media
              WHERE id = ?1 AND companion_of IS NULL AND state = 'normal'",
            [id],
            |r| {
                let path: String = r.get(3)?;
                Ok(Shot {
                    id,
                    album_id: r.get(0)?,
                    name: r.get(1)?,
                    captured_at: r.get(2)?,
                    raw: format_of(Path::new(&path)) == Some(MediaFormat::Raw),
                })
            },
        )
        .optional()?)
}

/// Le fichier de l'autre format qui porte le numéro voisin, dans le même
/// album, au même moment, et pas encore dans une pile.
fn partner(conn: &Connection, s: &Shot) -> Result<Option<i64>, LibraryError> {
    let Some((prefix, number, width)) = numbered(&s.name) else {
        return Ok(None);
    };
    let other = if s.raw {
        number + 1
    } else if number > 0 {
        number - 1
    } else {
        return Ok(None);
    };
    let pattern = format!("{prefix}{other:0width$}.%");
    let mut stmt = conn.prepare(
        "SELECT id, original_name, captured_at, path FROM media
          WHERE album_id = ?1 AND id <> ?2 AND original_name LIKE ?3
            AND companion_of IS NULL AND state = 'normal'
            AND NOT EXISTS (SELECT 1 FROM media c WHERE c.companion_of = media.id)",
    )?;
    let rows = stmt.query_map(params![s.album_id, s.id, pattern], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (id, name, captured_at, path) = row?;
        let raw = format_of(Path::new(&path)) == Some(MediaFormat::Raw);
        let companion = if s.raw { &name } else { &s.name };
        if raw != s.raw
            && pairs::is_companion(Path::new(companion))
            && pairs::same_moment(s.captured_at.as_deref(), captured_at.as_deref())
        {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// Met en pile les photos du boîtier qui viennent d'être importées avec leur
/// RAW ou leur JPEG ; rend le nombre de piles formées.
pub fn link_shots(conn: &Connection, media_ids: &[i64]) -> Result<usize, LibraryError> {
    let mut linked = 0;
    for &id in media_ids {
        let Some(s) = shot(conn, id)? else { continue };
        if s.raw
            && conn
                .query_row(
                    "SELECT 1 FROM media WHERE companion_of = ?1",
                    [s.id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some()
        {
            continue;
        }
        if let Some(other) = partner(conn, &s)? {
            let (raw, companion) = if s.raw { (s.id, other) } else { (other, s.id) };
            pairs::link(conn, raw, companion)?;
            linked += 1;
        }
    }
    Ok(linked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeros_des_fichiers() {
        assert_eq!(numbered("DSCF0001.raf"), Some(("DSCF", 1, 4)));
        assert_eq!(numbered("_DSF9999.JPG"), Some(("_DSF", 9999, 4)));
        assert_eq!(numbered("photo.jpg"), None);
    }
}
