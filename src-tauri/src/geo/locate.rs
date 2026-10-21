//! Pose des lieux en base, d'après des traces GPX ou des photos de téléphone.
//! Le GPS écrit par le boîtier n'est jamais remplacé ; un lieu déjà posé ne
//! l'est que sur demande. Chaque photo localisée part dans la file
//! d'écriture des XMP ; les fichiers GPX et de téléphone ne sont que lus.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::GeoError;
use super::gpx::{self, Track};
use super::matching::{self, Clock, Fix, Shot};
use super::naming;
use crate::db::now_utc;
use crate::library::pairs;
use crate::metadata::media::{format_of, read_metadata};
use crate::metadata::xmp::LocationSource;

/// Réglages communs aux deux sources.
#[derive(Debug, Clone, Copy)]
pub struct LocateOptions {
    pub clock: Clock,
    /// Remplacer un lieu déjà posé (trace, téléphone, saisie).
    pub replace: bool,
}

/// Bilan d'une localisation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocateReport {
    pub located: usize,
    /// Déjà localisées (GPS du boîtier, ou lieu gardé).
    pub kept: usize,
    /// Aucun point assez proche dans le temps.
    pub unmatched: usize,
    /// Heure ou fuseau inconnus.
    pub no_time: usize,
    /// Photos de téléphone sans heure ou sans position.
    pub unusable_sources: usize,
}

struct Candidate {
    shot: Shot,
    current: Option<LocationSource>,
}

fn candidates(conn: &Connection, ids: &[i64]) -> Result<Vec<Candidate>, GeoError> {
    // Une pile (RAW + JPEG) prend un seul lieu.
    let ids = pairs::expand(conn, ids, Some("normal"))?;
    let mut stmt = conn.prepare(
        "SELECT captured_at, utc_offset, location_source FROM media
         WHERE id = ?1 AND state = 'normal'",
    )?;
    let mut out = Vec::new();
    for id in ids {
        let row: Option<(Option<String>, Option<String>, Option<String>)> = stmt
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        let Some((captured, offset, source)) = row else {
            continue;
        };
        out.push(Candidate {
            shot: Shot {
                id,
                local: captured
                    .as_deref()
                    .and_then(matching::parse_local)
                    .unwrap_or_default(),
                offset: offset.as_deref().and_then(matching::parse_offset),
            },
            current: source.as_deref().and_then(LocationSource::from_db),
        });
    }
    Ok(out)
}

fn locate_with(
    conn: &mut Connection,
    ids: &[i64],
    options: LocateOptions,
    source: LocationSource,
    timed: bool,
    find: impl Fn(Option<DateTime<Utc>>) -> Option<(f64, f64)>,
) -> Result<LocateReport, GeoError> {
    let candidates = candidates(conn, ids)?;
    let tx = conn.transaction()?;
    let mut report = LocateReport::default();
    let now = now_utc();
    for c in candidates {
        match c.current {
            Some(LocationSource::Exif) => {
                report.kept += 1;
                continue;
            }
            Some(_) if !options.replace => {
                report.kept += 1;
                continue;
            }
            _ => {}
        }
        // Heure inconnue : `local` vaut l'époque, sans signification. La
        // saisie manuelle n'en a pas besoin.
        let t = c
            .shot
            .utc(options.clock)
            .filter(|_| c.shot.local != Default::default());
        if t.is_none() && timed {
            report.no_time += 1;
            continue;
        }
        let Some((lat, lon)) = find(t) else {
            report.unmatched += 1;
            continue;
        };
        tx.execute(
            "UPDATE media SET lat = ?2, lon = ?3, location_source = ?4,
                    xmp_pending = xmp_pending + 1, updated_at = ?5
             WHERE id = ?1",
            params![c.shot.id, lat, lon, source.as_db(), now],
        )?;
        naming::name_media(&tx, c.shot.id)?;
        report.located += 1;
    }
    tx.commit()?;
    Ok(report)
}

fn encode_points(points: &[Fix]) -> Vec<u8> {
    let mut out = Vec::with_capacity(points.len() * 24);
    for p in points {
        out.extend_from_slice(&p.time.timestamp().to_le_bytes());
        out.extend_from_slice(&p.latitude.to_le_bytes());
        out.extend_from_slice(&p.longitude.to_le_bytes());
    }
    out
}

/// Garde la trace pour la carte (une fois par bibliothèque).
fn store_track(conn: &Connection, library_id: i64, track: &Track) -> Result<(), GeoError> {
    let (Some(first), Some(last)) = (track.points.first(), track.points.last()) else {
        return Ok(());
    };
    conn.execute(
        "INSERT OR IGNORE INTO gpx_tracks
           (library_id, name, started_at, ended_at, points, imported_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            library_id,
            track.name,
            first.time.to_rfc3339(),
            last.time.to_rfc3339(),
            encode_points(&track.points),
            now_utc(),
        ],
    )?;
    Ok(())
}

fn library_of(conn: &Connection, ids: &[i64]) -> Result<Option<i64>, GeoError> {
    let Some(&id) = ids.first() else {
        return Ok(None);
    };
    Ok(conn
        .query_row("SELECT library_id FROM media WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?)
}

/// Localise des photos d'après une ou plusieurs traces GPX : chacune prend
/// la position de la trace à son heure (voir [`matching::on_track`]).
pub fn locate_gpx(
    conn: &mut Connection,
    ids: &[i64],
    files: &[PathBuf],
    options: LocateOptions,
    max_gap: Duration,
) -> Result<LocateReport, GeoError> {
    let tracks = files
        .iter()
        .map(|f| gpx::read(f).map_err(|e| GeoError::File(f.clone(), e.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    let mut points: Vec<Fix> = tracks.iter().flat_map(|t| t.points.clone()).collect();
    points.sort_by_key(|p| p.time);
    let report = locate_with(conn, ids, options, LocationSource::Gpx, true, |t| {
        matching::on_track(&points, t?, max_gap)
    })?;
    if let Some(library_id) = library_of(conn, ids)? {
        for t in &tracks {
            store_track(conn, library_id, t)?;
        }
    }
    Ok(report)
}

/// Position et instant d'une photo de téléphone ; son heure locale est
/// ramenée en UTC par le fuseau qu'il a écrit, sinon par `clock`.
fn phone_fix(path: &Path, clock: Clock) -> Option<Fix> {
    let meta = read_metadata(path, format_of(path)?);
    let gps = meta.gps?;
    let shot = Shot {
        id: 0,
        local: matching::parse_local(meta.captured_at.as_deref()?)?,
        offset: meta.utc_offset.as_deref().and_then(matching::parse_offset),
    };
    let time = shot.utc(Clock::Written).or_else(|| shot.utc(clock))?;
    Some(Fix {
        time,
        latitude: gps.latitude,
        longitude: gps.longitude,
    })
}

/// Localise des photos d'après des photos de téléphone : chacune prend la
/// position de la plus proche dans le temps, à moins de `window`.
pub fn locate_phone(
    conn: &mut Connection,
    ids: &[i64],
    files: &[PathBuf],
    options: LocateOptions,
    window: Duration,
) -> Result<LocateReport, GeoError> {
    let fixes: Vec<Fix> = files
        .iter()
        .filter_map(|f| phone_fix(f, options.clock))
        .collect();
    let unusable = files.len() - fixes.len();
    let mut report = locate_with(conn, ids, options, LocationSource::Phone, true, |t| {
        matching::nearest(&fixes, t?, window)
    })?;
    report.unusable_sources = unusable;
    Ok(report)
}

/// Pose un lieu saisi (ville cherchée ou point de la carte) sur des photos.
pub fn locate_manual(
    conn: &mut Connection,
    ids: &[i64],
    latitude: f64,
    longitude: f64,
    replace: bool,
) -> Result<LocateReport, GeoError> {
    let options = LocateOptions {
        clock: Clock::Written,
        replace,
    };
    locate_with(conn, ids, options, LocationSource::Manual, false, |_| {
        Some((latitude, longitude))
    })
}

/// Photos d'un import sans lieu (une par pile), à proposer à la
/// localisation à la fin de l'import.
pub fn unlocated_in_import(conn: &Connection, import_id: i64) -> Result<Vec<i64>, GeoError> {
    let ids = conn
        .prepare(
            "SELECT id FROM media
             WHERE import_id = ?1 AND state = 'normal' AND lat IS NULL
               AND companion_of IS NULL
             ORDER BY captured_at, id",
        )?
        .query_map([import_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}
