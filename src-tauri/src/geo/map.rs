//! Ce que montre la carte du monde (cahier des charges, 4.7) : chaque album
//! en un point, ses photos une à une en zoomant, et les traces GPX de ses
//! jours.

use std::collections::HashMap;

use rusqlite::{Connection, params};
use serde::Serialize;

use super::GeoError;

/// Un album sur la carte.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumPoint {
    pub album_id: i64,
    pub name: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    /// Point central de ses photos localisées (médianes).
    pub latitude: f64,
    pub longitude: f64,
    /// Photos localisées, sur toutes celles de l'album.
    pub located: usize,
    pub total: usize,
    /// Couverture choisie si elle est localisée, sinon sa première photo
    /// localisée.
    pub cover_media_id: i64,
    /// Ville la plus fréquente parmi ses photos, et son pays.
    pub city: Option<String>,
    pub country: Option<String>,
}

/// Une photo localisée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPoint {
    pub id: i64,
    pub album_id: i64,
    pub latitude: f64,
    pub longitude: f64,
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// Photos localisées d'une bibliothèque (une par pile), dans l'ordre des
/// prises de vue.
pub fn media_points(conn: &Connection, library_id: i64) -> Result<Vec<MediaPoint>, GeoError> {
    let points = conn
        .prepare(
            "SELECT id, album_id, lat, lon FROM media
             WHERE library_id = ?1 AND state = 'normal' AND lat IS NOT NULL
               AND companion_of IS NULL
             ORDER BY captured_at, id",
        )?
        .query_map([library_id], |r| {
            Ok(MediaPoint {
                id: r.get(0)?,
                album_id: r.get(1)?,
                latitude: r.get(2)?,
                longitude: r.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(points)
}

/// Les albums qui ont au moins une photo localisée.
pub fn album_points(conn: &Connection, library_id: i64) -> Result<Vec<AlbumPoint>, GeoError> {
    struct Acc {
        lats: Vec<f64>,
        lons: Vec<f64>,
        first: i64,
        ids: Vec<i64>,
        cities: HashMap<(String, String), usize>,
    }
    let mut by_album: HashMap<i64, Acc> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT album_id, id, lat, lon, city, country FROM media
         WHERE library_id = ?1 AND state = 'normal' AND lat IS NOT NULL
           AND companion_of IS NULL
         ORDER BY captured_at, id",
    )?;
    let rows = stmt.query_map([library_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, f64>(2)?,
            r.get::<_, f64>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    for row in rows {
        let (album, id, lat, lon, city, country) = row?;
        let acc = by_album.entry(album).or_insert_with(|| Acc {
            lats: Vec::new(),
            lons: Vec::new(),
            first: id,
            ids: Vec::new(),
            cities: HashMap::new(),
        });
        acc.lats.push(lat);
        acc.lons.push(lon);
        acc.ids.push(id);
        if let (Some(city), Some(country)) = (city, country) {
            *acc.cities.entry((city, country)).or_default() += 1;
        }
    }
    let mut stmt = conn.prepare(
        "SELECT a.name, a.start_date, a.end_date, a.cover_media_id,
                (SELECT count(*) FROM media m
                  WHERE m.album_id = a.id AND m.state = 'normal' AND m.companion_of IS NULL)
           FROM albums a WHERE a.id = ?1",
    )?;
    let mut out = Vec::new();
    for (album_id, mut acc) in by_album {
        let (name, start_date, end_date, cover, total): (
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            i64,
        ) = stmt.query_row(params![album_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?;
        let place = acc
            .cities
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(k, _)| k.clone());
        out.push(AlbumPoint {
            album_id,
            name,
            start_date,
            end_date,
            latitude: median(&mut acc.lats),
            longitude: median(&mut acc.lons),
            located: acc.ids.len(),
            total: usize::try_from(total).unwrap_or(0),
            cover_media_id: cover.filter(|c| acc.ids.contains(c)).unwrap_or(acc.first),
            city: place.as_ref().map(|p| p.0.clone()),
            country: place.map(|p| p.1).filter(|c| !c.is_empty()),
        });
    }
    out.sort_by(|a, b| {
        b.start_date
            .cmp(&a.start_date)
            .then(a.album_id.cmp(&b.album_id))
    });
    Ok(out)
}

/// Traces GPX qui recouvrent les jours d'un album (AAAA-MM-JJ, bornes
/// comprises, à un jour près pour les fuseaux), en lignes de points
/// [longitude, latitude].
pub fn album_tracks(
    conn: &Connection,
    library_id: i64,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<Vec<[f64; 2]>>, GeoError> {
    let mut stmt = conn.prepare(
        "SELECT points FROM gpx_tracks
         WHERE library_id = ?1
           AND substr(started_at, 1, 10) <= date(?3, '+1 day')
           AND substr(ended_at, 1, 10) >= date(?2, '-1 day')
         ORDER BY started_at",
    )?;
    let tracks = stmt
        .query_map(params![library_id, start_date, end_date], |r| {
            r.get::<_, Vec<u8>>(0)
        })?
        .map(|blob| {
            blob.map(|b| {
                b.as_chunks::<24>()
                    .0
                    .iter()
                    .map(|p| {
                        let f =
                            |o: usize| f64::from_le_bytes(p[o..o + 8].try_into().unwrap_or([0; 8]));
                        [f(16), f(8)]
                    })
                    .collect()
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mediane_robuste_aux_photos_isolees() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [1.0, 2.0, 3.0, 100.0]), 2.5);
    }
}
