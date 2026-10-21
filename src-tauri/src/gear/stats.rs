//! Statistiques d'usage d'un boîtier ou d'un objectif (cahier des charges,
//! 4.8) : focales, ouvertures, ISO, photos par mois, matériel associé,
//! albums et compteur de déclenchements.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use std::path::Path;

use super::{GearError, GearKind, SELECT_ITEM, read_item};
use crate::library::paths;
use crate::metadata::media::{format_of, read_metadata};

/// Photos prises avec l'appareil `?1`, sans les JPEG compagnons d'un RAW
/// (une seule prise de vue) ni la corbeille.
const USED: &str = "(m.camera_id = ?1 OR m.lens_id = ?1)
    AND m.state = 'normal' AND m.companion_of IS NULL";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValueCount {
    pub value: f64,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonthCount {
    /// `AAAA-MM`.
    pub month: String,
    pub count: i64,
}

/// Objectif utilisé avec un boîtier, ou boîtier avec un objectif.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Partner {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumUse {
    pub album_id: i64,
    pub name: String,
    pub count: i64,
    /// Objectif le plus utilisé dans l'album (boîtiers seulement).
    pub favorite_lens: Option<String>,
}

/// Déclenchements : la dernière valeur lue dans une photo, plus les photos
/// prises depuis ; sans valeur lue, le nombre de photos importées.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutterCount {
    pub read: Option<i64>,
    pub read_at: Option<String>,
    pub since: i64,
    pub estimate: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearStats {
    pub photos: i64,
    pub focal: Vec<ValueCount>,
    pub aperture: Vec<ValueCount>,
    pub iso: Vec<ValueCount>,
    pub months: Vec<MonthCount>,
    pub partners: Vec<Partner>,
    pub albums: Vec<AlbumUse>,
    pub shutter: Option<ShutterCount>,
}

fn values(conn: &Connection, id: i64, expr: &str) -> Result<Vec<ValueCount>, GearError> {
    let sql = format!(
        "SELECT {expr} AS v, count(*) FROM media m JOIN media_exif e ON e.media_id = m.id
          WHERE {USED} AND v > 0 GROUP BY v ORDER BY v"
    );
    let rows = conn
        .prepare(&sql)?
        .query_map([id], |r| {
            Ok(ValueCount {
                value: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

fn display_name(conn: &Connection, id: i64) -> Result<Option<String>, GearError> {
    Ok(conn
        .query_row(&format!("{SELECT_ITEM} WHERE g.id = ?1"), [id], read_item)
        .optional()?
        .map(|g| g.display_name))
}

/// Photos importées avant que le compteur ne soit lu : relève celui de la
/// plus récente, en lecture seule, chez les marques qui l'écrivent.
fn fill_shutter(conn: &Connection, id: i64, make: &str) -> Result<(), GearError> {
    let make = make.to_lowercase();
    if !(make.contains("nikon") || make.contains("fujifilm")) {
        return Ok(());
    }
    let latest: Option<(i64, String, String, Option<i64>)> = conn
        .query_row(
            &format!(
                "SELECT m.id, l.root_path, m.path, e.shutter_count
                   FROM media m JOIN media_exif e ON e.media_id = m.id
                   JOIN libraries l ON l.id = m.library_id
                  WHERE {USED} AND m.captured_at IS NOT NULL
                  ORDER BY m.captured_at DESC LIMIT 1"
            ),
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((media_id, root, rel, None)) = latest else {
        return Ok(());
    };
    // Original absent (disque débranché) ou sans compteur : rien à relever.
    let Ok(abs) = paths::to_absolute(Path::new(&root), &rel) else {
        return Ok(());
    };
    let Some(format) = format_of(&abs) else {
        return Ok(());
    };
    if let Some(count) = read_metadata(&abs, format).shutter_count {
        conn.execute(
            "UPDATE media_exif SET shutter_count = ?2 WHERE media_id = ?1",
            params![media_id, count],
        )?;
    }
    Ok(())
}

pub fn stats(conn: &Connection, id: i64) -> Result<GearStats, GearError> {
    let item = super::get(conn, id)?.item;
    let kind = item.kind;
    if kind == GearKind::Body {
        fill_shutter(conn, id, &item.make)?;
    }
    let photos: i64 = conn.query_row(
        &format!("SELECT count(*) FROM media m WHERE {USED}"),
        [id],
        |r| r.get(0),
    )?;

    let months = conn
        .prepare(&format!(
            "SELECT substr(m.captured_at, 1, 7) AS month, count(*) FROM media m
              WHERE {USED} AND m.captured_at IS NOT NULL GROUP BY month ORDER BY month"
        ))?
        .query_map([id], |r| {
            Ok(MonthCount {
                month: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    // Un boîtier a ses objectifs, un objectif ses boîtiers ; un accessoire
    // n'apparaît dans aucune photo.
    let other = match kind {
        GearKind::Body => Some("lens_id"),
        GearKind::Lens => Some("camera_id"),
        _ => None,
    };
    let mut partners = Vec::new();
    if let Some(col) = other {
        let rows: Vec<(i64, i64)> = conn
            .prepare(&format!(
                "SELECT m.{col}, count(*) AS n FROM media m
                  WHERE {USED} AND m.{col} IS NOT NULL AND m.{col} != ?1
                  GROUP BY m.{col} ORDER BY n DESC LIMIT 8"
            ))?
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (pid, count) in rows {
            if let Some(name) = display_name(conn, pid)? {
                partners.push(Partner {
                    id: pid,
                    name,
                    count,
                });
            }
        }
    }

    let top_albums: Vec<(i64, String, i64)> = conn
        .prepare(&format!(
            "SELECT a.id, a.name, count(*) AS n FROM media m JOIN albums a ON a.id = m.album_id
              WHERE {USED} GROUP BY a.id ORDER BY n DESC, a.start_date DESC LIMIT 8"
        ))?
        .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut albums = Vec::with_capacity(top_albums.len());
    for (album_id, name, count) in top_albums {
        let favorite_lens = if kind == GearKind::Body {
            let lens: Option<i64> = conn
                .query_row(
                    &format!(
                        "SELECT m.lens_id FROM media m
                          WHERE {USED} AND m.album_id = ?2 AND m.lens_id IS NOT NULL
                          GROUP BY m.lens_id ORDER BY count(*) DESC LIMIT 1"
                    ),
                    params![id, album_id],
                    |r| r.get(0),
                )
                .optional()?;
            match lens {
                Some(l) => display_name(conn, l)?,
                None => None,
            }
        } else {
            None
        };
        albums.push(AlbumUse {
            album_id,
            name,
            count,
            favorite_lens,
        });
    }

    let shutter = if kind == GearKind::Body {
        let read: Option<(i64, Option<String>)> = conn
            .query_row(
                &format!(
                    "SELECT e.shutter_count, m.captured_at
                       FROM media m JOIN media_exif e ON e.media_id = m.id
                      WHERE {USED} AND e.shutter_count IS NOT NULL
                      ORDER BY e.shutter_count DESC LIMIT 1"
                ),
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let since: i64 = match &read {
            Some((_, Some(at))) => conn.query_row(
                &format!("SELECT count(*) FROM media m WHERE {USED} AND m.captured_at > ?2"),
                params![id, at],
                |r| r.get(0),
            )?,
            _ => 0,
        };
        Some(ShutterCount {
            estimate: read.as_ref().map_or(photos, |(n, _)| n + since),
            read: read.as_ref().map(|(n, _)| *n),
            read_at: read.and_then(|(_, at)| at),
            since,
        })
    } else {
        None
    };

    Ok(GearStats {
        photos,
        focal: values(conn, id, "round(e.focal_length)")?,
        aperture: values(conn, id, "round(e.aperture, 1)")?,
        iso: values(conn, id, "round(e.iso)")?,
        months,
        partners,
        albums,
        shutter,
    })
}
