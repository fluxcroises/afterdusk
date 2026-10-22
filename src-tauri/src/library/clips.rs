//! Dérushage vidéo (cahier des charges, 4.9) : extraits d'un clip, gardés en
//! base. Le début de chaque extrait est recalé sur l'image clé qui le
//! précède, pour que l'export sans réencodage commence exactement là.

use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub media_id: i64,
    pub start_ms: u64,
    pub end_ms: u64,
    /// Vide : l'extrait est nommé d'après ses instants.
    pub name: String,
}

/// Extraits d'une vidéo, dans l'ordre du clip.
pub fn list(conn: &Connection, media_id: i64) -> rusqlite::Result<Vec<Clip>> {
    conn.prepare(
        "SELECT id, media_id, start_ms, end_ms, name FROM video_clips
          WHERE media_id = ?1 ORDER BY start_ms, id",
    )?
    .query_map([media_id], |r| {
        Ok(Clip {
            id: r.get(0)?,
            media_id: r.get(1)?,
            start_ms: r.get::<_, i64>(2)?.max(0) as u64,
            end_ms: r.get::<_, i64>(3)?.max(0) as u64,
            name: r.get(4)?,
        })
    })?
    .collect()
}

/// Nouvel extrait ; `start_ms` est recalé sur l'image clé qui le précède.
/// `None` si l'extrait serait vide.
pub fn add(
    conn: &Connection,
    media_id: i64,
    keyframes: &[u64],
    start_ms: u64,
    end_ms: u64,
) -> rusqlite::Result<Option<Clip>> {
    let start = crate::video::snap(keyframes, start_ms);
    if end_ms <= start {
        return Ok(None);
    }
    conn.execute(
        "INSERT INTO video_clips (media_id, start_ms, end_ms, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![media_id, start as i64, end_ms as i64, crate::db::now_utc()],
    )?;
    Ok(Some(Clip {
        id: conn.last_insert_rowid(),
        media_id,
        start_ms: start,
        end_ms,
        name: String::new(),
    }))
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE video_clips SET name = ?2 WHERE id = ?1",
        params![id, name.trim()],
    )?;
    Ok(())
}

/// Retire un extrait : rien n'est touché dans la vidéo.
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM video_clips WHERE id = ?1", [id])?;
    Ok(())
}

/// Instant lisible pour un nom de fichier : « 01m05s », « 1h02m05s ».
pub fn stamp(ms: u64) -> String {
    let s = ms / 1000;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}h{m:02}m{s:02}s")
    } else {
        format!("{m:02}m{s:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(conn: &Connection) -> i64 {
        conn.execute_batch(
            "INSERT INTO libraries (id, name, root_path, created_at) VALUES (1, 'V', '/v', 'x');
             INSERT INTO albums (id, library_id, name, folder, created_at)
               VALUES (1, 1, 'A', 'a', 'x');
             INSERT INTO media (id, library_id, album_id, kind, path, original_name, file_size,
                                fingerprint, content_hash, imported_at, updated_at)
               VALUES (5, 1, 1, 'video', 'a/C0001.MP4', 'C0001.MP4', 1, 'f', 'h', 'x', 'x');",
        )
        .unwrap();
        5
    }

    #[test]
    fn extraits_recales_et_ordonnes() {
        let conn = crate::db::open_in_memory().unwrap();
        let id = video(&conn);
        let keys = [0, 480, 960, 1440];
        let b = add(&conn, id, &keys, 1500, 3000).unwrap().unwrap();
        let a = add(&conn, id, &keys, 1000, 1200).unwrap().unwrap();
        assert_eq!((a.start_ms, b.start_ms), (960, 1440));
        // Fin avant l'image clé de départ : rien.
        assert!(add(&conn, id, &keys, 1000, 900).unwrap().is_none());
        rename(&conn, a.id, "  Discours ").unwrap();
        let all = list(&conn, id).unwrap();
        assert_eq!(all.iter().map(|c| c.id).collect::<Vec<_>>(), [a.id, b.id]);
        assert_eq!(all[0].name, "Discours");
        delete(&conn, b.id).unwrap();
        assert_eq!(list(&conn, id).unwrap().len(), 1);
    }

    #[test]
    fn instants_lisibles() {
        assert_eq!(stamp(65_400), "01m05s");
        assert_eq!(stamp(3_725_000), "1h02m05s");
    }
}
