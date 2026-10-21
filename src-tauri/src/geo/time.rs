//! Heure du boîtier et heure locale du lieu (cahier des charges, 4.7) : un
//! boîtier resté à l'heure de Paris à New York date ses photos six heures
//! trop tard. Le fuseau du lieu (géocodage) et celui de l'horloge du boîtier
//! (écrit par lui, ou saisi) donnent la correction, appliquée sur demande :
//! la base et le XMP changent, jamais l'original.

use chrono::{DateTime, FixedOffset, NaiveDateTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::GeoError;
use super::matching::{Clock, parse_local, parse_offset};
use crate::db::now_utc;
use crate::library::pairs;

/// Décalage du fuseau IANA `zone` sur UTC à cet instant (heure d'été comprise).
pub fn zone_offset(zone: &str, at: DateTime<Utc>) -> Option<FixedOffset> {
    let tz: Tz = zone.parse().ok()?;
    Some(tz.offset_from_utc_datetime(&at.naive_utc()).fix())
}

/// « +02:00 ».
pub fn format_offset(offset: FixedOffset) -> String {
    let s = offset.local_minus_utc();
    let sign = if s < 0 { '-' } else { '+' };
    let m = s.abs() / 60;
    format!("{sign}{:02}:{:02}", m / 60, m % 60)
}

/// Photos prises au même endroit avec la même erreur d'horloge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeGroup {
    pub city: Option<String>,
    /// Fuseau de l'horloge du boîtier, en minutes sur UTC.
    pub camera_minutes: i32,
    /// Fuseau du lieu à la date des photos.
    pub local_minutes: i32,
    /// Correction à appliquer à l'heure du boîtier.
    pub shift_minutes: i32,
    pub count: usize,
}

/// Bilan des photos examinées.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeReport {
    pub groups: Vec<TimeGroup>,
    /// Déjà à l'heure locale.
    pub correct: usize,
    /// Sans lieu nommé (pas de fuseau du lieu).
    pub no_place: usize,
    /// Fuseau de l'horloge inconnu (ni écrit ni saisi).
    pub no_zone: usize,
    pub no_time: usize,
}

/// Heure, fuseau écrit, fuseau du lieu et ville d'une photo.
type Row = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

struct Fix {
    id: i64,
    local: NaiveDateTime,
    offset: FixedOffset,
}

fn examine(
    conn: &Connection,
    ids: &[i64],
    clock: Clock,
) -> Result<(Vec<Fix>, TimeReport), GeoError> {
    let ids = pairs::expand(conn, ids, Some("normal"))?;
    let mut stmt = conn.prepare(
        "SELECT captured_at, utc_offset, timezone, city FROM media
         WHERE id = ?1 AND state = 'normal'",
    )?;
    let mut fixes = Vec::new();
    let mut report = TimeReport::default();
    for id in ids {
        let row: Option<Row> = stmt
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()?;
        let Some((captured, written, zone, city)) = row else {
            continue;
        };
        let Some(local) = captured.as_deref().and_then(parse_local) else {
            report.no_time += 1;
            continue;
        };
        let Some(zone) = zone.filter(|z| !z.is_empty()) else {
            report.no_place += 1;
            continue;
        };
        let camera = match clock {
            Clock::Written => written.as_deref().and_then(parse_offset),
            Clock::Offset(d) => i32::try_from(d.num_seconds())
                .ok()
                .and_then(FixedOffset::east_opt),
        };
        let Some(camera) = camera else {
            report.no_zone += 1;
            continue;
        };
        let Some(utc) = camera
            .from_local_datetime(&local)
            .single()
            .map(|t| t.with_timezone(&Utc))
        else {
            report.no_time += 1;
            continue;
        };
        let Some(place) = zone_offset(&zone, utc) else {
            report.no_place += 1;
            continue;
        };
        if place == camera {
            report.correct += 1;
            continue;
        }
        let camera_minutes = camera.local_minus_utc() / 60;
        let local_minutes = place.local_minus_utc() / 60;
        match report.groups.iter_mut().find(|g| {
            g.city == city && g.camera_minutes == camera_minutes && g.local_minutes == local_minutes
        }) {
            Some(g) => g.count += 1,
            None => report.groups.push(TimeGroup {
                city,
                camera_minutes,
                local_minutes,
                shift_minutes: local_minutes - camera_minutes,
                count: 1,
            }),
        }
        fixes.push(Fix {
            id,
            local: utc.with_timezone(&place).naive_local(),
            offset: place,
        });
    }
    report.groups.sort_by_key(|g| std::cmp::Reverse(g.count));
    Ok((fixes, report))
}

/// Ce qu'une correction changerait, sans rien modifier.
pub fn proposals(conn: &Connection, ids: &[i64], clock: Clock) -> Result<TimeReport, GeoError> {
    Ok(examine(conn, ids, clock)?.1)
}

/// Met l'heure des photos à l'heure locale du lieu. L'heure du boîtier est
/// gardée la première fois (`captured_at_camera`) ; le XMP suit.
pub fn correct(conn: &mut Connection, ids: &[i64], clock: Clock) -> Result<usize, GeoError> {
    let (fixes, _) = examine(conn, ids, clock)?;
    let tx = conn.transaction()?;
    let now = now_utc();
    for f in &fixes {
        tx.execute(
            "UPDATE media SET captured_at_camera = coalesce(captured_at_camera, captured_at),
                    captured_at = ?2, utc_offset = ?3,
                    xmp_pending = xmp_pending + 1, updated_at = ?4
             WHERE id = ?1",
            params![
                f.id,
                f.local.format("%Y-%m-%dT%H:%M:%S").to_string(),
                format_offset(f.offset),
                now
            ],
        )?;
    }
    tx.commit()?;
    Ok(fixes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuseau_du_lieu_heure_d_ete_comprise() {
        let summer = Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap();
        let winter = Utc.with_ymd_and_hms(2026, 1, 14, 12, 0, 0).unwrap();
        let minutes = |z, t| zone_offset(z, t).unwrap().local_minus_utc() / 60;
        assert_eq!(minutes("Europe/Paris", summer), 120);
        assert_eq!(minutes("Europe/Paris", winter), 60);
        assert_eq!(minutes("America/New_York", summer), -240);
        assert_eq!(minutes("Asia/Kolkata", winter), 330);
        assert!(zone_offset("Pas/Un_Fuseau", summer).is_none());
        assert_eq!(
            format_offset(FixedOffset::east_opt(-19800).unwrap()),
            "-05:30"
        );
    }
}
