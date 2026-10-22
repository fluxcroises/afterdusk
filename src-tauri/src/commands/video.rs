//! Lecture des vidéos dans l'interface (décision 0012) : protocole `video://`
//! qui sert l'original, ou sa version de lecture H.264 quand le lecteur
//! intégré ne sait pas lire le codec, par plages d'octets (le lecteur saute
//! dans le clip sans tout charger). L'original n'est qu'ouvert en lecture.
//!
//! - `video://localhost/media/<id>` : l'original ;
//! - `video://localhost/proxy/<id>` : la version de lecture, en cache.
//!
//! Sous Windows, la même adresse s'écrit `http://video.localhost/…`.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, State, UriSchemeContext, Wry};

use super::error::{CommandError, CommandResult};
use super::{AppState, blocking};
use crate::library::{get_library, paths};
use crate::video;

pub const SCHEME: &str = "video";
/// Morceau servi quand le lecteur demande « jusqu'à la fin ».
const CHUNK: u64 = 4 * 1024 * 1024;

/// Version de lecture d'un média, dans le cache.
pub fn proxy_path(cache_root: &Path, media_id: i64) -> PathBuf {
    cache_root.join("video").join(format!("{media_id}.mp4"))
}

/// Original d'une vidéo, sa bibliothèque étant branchée.
fn original(conn: &Connection, media_id: i64) -> Option<PathBuf> {
    let (library_id, rel): (i64, String) = conn
        .query_row(
            "SELECT library_id, path FROM media WHERE id = ?1 AND kind = 'video'",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .ok()??;
    let lib = get_library(conn, library_id).ok()?;
    if !lib.is_online {
        return None;
    }
    paths::to_absolute(&lib.root_path, &rel).ok()
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        // Les MOV des boîtiers sont des MP4 : le lecteur de Chromium les lit ainsi.
        Some("mp4" | "mov" | "m4v") => "video/mp4",
        Some("mts" | "m2ts") => "video/mp2t",
        _ => "application/octet-stream",
    }
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    response
}

/// Plage demandée (`bytes=début-fin`), bornée à la taille du fichier.
fn range(header_value: Option<&str>, size: u64) -> Option<(u64, u64)> {
    let spec = header_value?.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            (size.saturating_sub(n), size.saturating_sub(1))
        }
        (s, "") => {
            let s: u64 = s.parse().ok()?;
            (s, (s + CHUNK - 1).min(size.saturating_sub(1)))
        }
        (s, e) => (
            s.parse().ok()?,
            e.parse::<u64>().ok()?.min(size.saturating_sub(1)),
        ),
    };
    (start <= end && start < size).then_some((start, end))
}

fn serve(path: &Path, range_header: Option<&str>) -> Response<Vec<u8>> {
    let Ok(mut file) = std::fs::File::open(path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let (start, end, partial) = match range(range_header, size) {
        Some((s, e)) => (s, e, true),
        None if range_header.is_some() => return status(StatusCode::RANGE_NOT_SATISFIABLE),
        None => (0, size.min(CHUNK).saturating_sub(1), size > CHUNK),
    };
    let mut body = vec![0u8; (end - start + 1) as usize];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut body).is_err() {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, content_type(path))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, body.len().to_string());
    builder = if partial {
        builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
    } else {
        builder.status(StatusCode::OK)
    };
    builder
        .body(body)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

/// Protocole `video://` ; lecture du disque hors du fil de l'interface.
pub fn protocol(
    ctx: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let path = request.uri().path().trim_start_matches('/').to_owned();
        let range_header = request
            .headers()
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let file = match path.split_once('/') {
            Some(("media", id)) => id
                .parse()
                .ok()
                .and_then(|id| state.connect().ok().and_then(|c| original(&c, id))),
            Some(("proxy", id)) => id
                .parse()
                .ok()
                .map(|id| proxy_path(&state.cache_root, id))
                .filter(|p| p.is_file()),
            _ => None,
        };
        responder.respond(match file {
            Some(file) => serve(&file, range_header.as_deref()),
            None => status(StatusCode::NOT_FOUND),
        });
    });
}

/// Ce que l'interface doit lire pour une vidéo.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSource {
    /// Chemin à donner au protocole : `media/<id>` ou `proxy/<id>`.
    pub path: Option<String>,
    /// Le codec demande une version de lecture, pas encore prête.
    pub needs_proxy: bool,
    pub fps: f64,
    pub duration_ms: i64,
    pub codec: String,
    /// FFmpeg absent : ni informations ni version de lecture.
    pub ffmpeg: bool,
}

#[tauri::command]
pub async fn video_source(state: State<'_, AppState>, media_id: i64) -> CommandResult<VideoSource> {
    let conn = state.connect()?;
    let cache = state.cache_root.clone();
    blocking(move || {
        let info: Option<(f64, i64, String)> = conn
            .query_row(
                "SELECT coalesce(fps, 0), duration_ms, coalesce(codec, '') FROM media_video
                  WHERE media_id = ?1",
                [media_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(crate::library::LibraryError::from)?;
        let (fps, duration_ms, codec) = info.unwrap_or_default();
        let original = original(&conn, media_id);
        let native = original
            .as_deref()
            .is_some_and(|p| content_type(p) == "video/mp4" && video::plays_natively(&codec));
        let proxy_ready = proxy_path(&cache, media_id).is_file();
        let path = if native {
            Some(format!("media/{media_id}"))
        } else if proxy_ready {
            Some(format!("proxy/{media_id}"))
        } else {
            None
        };
        Ok(VideoSource {
            needs_proxy: path.is_none() && original.is_some(),
            path,
            fps,
            duration_ms,
            codec,
            ffmpeg: video::available(),
        })
    })
    .await
}

/// Prépare la version de lecture H.264 (peut prendre du temps sur un long clip).
#[tauri::command]
pub async fn video_prepare(state: State<'_, AppState>, media_id: i64) -> CommandResult<()> {
    let conn = state.connect()?;
    let cache = state.cache_root.clone();
    blocking(move || {
        let source = original(&conn, media_id)
            .ok_or_else(|| CommandError::new("videoUnavailable", media_id))?;
        let out = proxy_path(&cache, media_id);
        if out.is_file() {
            return Ok(());
        }
        std::fs::create_dir_all(out.parent().unwrap_or(&cache))
            .map_err(|e| CommandError::new("videoUnavailable", e))?;
        if video::proxy(&source, &out) {
            Ok(())
        } else {
            Err(CommandError::new("videoUnavailable", source.display()))
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plages_demandees_par_le_lecteur() {
        let size = 10_000_000;
        assert_eq!(range(Some("bytes=0-99"), size), Some((0, 99)));
        assert_eq!(
            range(Some("bytes=100-"), size),
            Some((100, 100 + CHUNK - 1))
        );
        assert_eq!(
            range(Some("bytes=-500"), size),
            Some((size - 500, size - 1))
        );
        assert_eq!(
            range(Some("bytes=9999990-20000000"), size),
            Some((9_999_990, size - 1))
        );
        assert_eq!(range(Some("bytes=20000000-"), size), None);
        assert_eq!(range(Some("lignes=0-1"), size), None);
    }
}
