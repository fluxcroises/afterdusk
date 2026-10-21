//! Localiser des photos : trace GPX ou photos de téléphone du même moment.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Duration;
use serde::Serialize;
use tauri::State;

use super::error::{CommandError, CommandResult};
use super::marks::XmpWriter;
use super::{AppState, blocking};
use crate::geo::GeoError;
use crate::geo::locate::{self, LocateOptions, LocateReport};
use crate::geo::matching::{Clock, parse_offset};

impl From<GeoError> for CommandError {
    fn from(e: GeoError) -> Self {
        let code = match e {
            GeoError::Gpx(_) | GeoError::File(..) => "gpxUnreadable",
            _ => "internal",
        };
        Self::new(code, e)
    }
}

/// Heure des photos : le fuseau écrit par le boîtier, s'il y en a un.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockInfo {
    /// Fuseau le plus fréquent dans la sélection, en minutes.
    pub written_minutes: Option<i32>,
    /// Les photos portent des fuseaux différents.
    pub mixed: bool,
    /// Photos qui portent un fuseau, sur le total.
    pub with_offset: usize,
    pub total: usize,
}

#[tauri::command]
pub async fn geo_clock(state: State<'_, AppState>, ids: Vec<i64>) -> CommandResult<ClockInfo> {
    let conn = state.connect()?;
    blocking(move || {
        let mut counts: HashMap<i32, usize> = HashMap::new();
        let mut total = 0;
        let mut stmt = conn
            .prepare("SELECT utc_offset FROM media WHERE id = ?1")
            .map_err(GeoError::from)?;
        for id in ids.into_iter().filter(|id| *id > 0) {
            let offset: Option<String> = stmt.query_row([id], |r| r.get(0)).unwrap_or(None);
            total += 1;
            if let Some(o) = offset.as_deref().and_then(parse_offset) {
                *counts.entry(o.local_minus_utc() / 60).or_default() += 1;
            }
        }
        Ok(ClockInfo {
            written_minutes: counts.iter().max_by_key(|(_, n)| **n).map(|(m, _)| *m),
            mixed: counts.len() > 1,
            with_offset: counts.values().sum(),
            total,
        })
    })
    .await
}

fn options(offset_minutes: Option<i32>, replace: bool) -> LocateOptions {
    LocateOptions {
        clock: offset_minutes.map_or(Clock::Written, |m| {
            Clock::Offset(Duration::minutes(i64::from(m)))
        }),
        replace,
    }
}

/// `offset_minutes` : heure du boîtier = UTC + ce décalage ; absent, le
/// fuseau écrit par le boîtier dans chaque photo.
#[tauri::command]
pub async fn geo_locate_gpx(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    ids: Vec<i64>,
    files: Vec<PathBuf>,
    offset_minutes: Option<i32>,
    max_gap_minutes: u32,
    replace: bool,
) -> CommandResult<LocateReport> {
    let mut conn = state.connect()?;
    let ids: Vec<i64> = ids.into_iter().filter(|id| *id > 0).collect();
    let report = blocking(move || {
        Ok(locate::locate_gpx(
            &mut conn,
            &ids,
            &files,
            options(offset_minutes, replace),
            Duration::minutes(i64::from(max_gap_minutes)),
        )?)
    })
    .await?;
    writer.wake();
    Ok(report)
}

#[tauri::command]
pub async fn geo_locate_phone(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    ids: Vec<i64>,
    files: Vec<PathBuf>,
    offset_minutes: Option<i32>,
    window_minutes: u32,
    replace: bool,
) -> CommandResult<LocateReport> {
    let mut conn = state.connect()?;
    let ids: Vec<i64> = ids.into_iter().filter(|id| *id > 0).collect();
    let report = blocking(move || {
        Ok(locate::locate_phone(
            &mut conn,
            &ids,
            &files,
            options(offset_minutes, replace),
            Duration::minutes(i64::from(window_minutes)),
        )?)
    })
    .await?;
    writer.wake();
    Ok(report)
}
