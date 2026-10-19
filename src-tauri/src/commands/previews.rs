//! Aperçus : file de génération en arrière-plan et protocole `preview://`
//! qui sert les images à l'interface.
//!
//! - `preview://localhost/media/<id>/<taille>` : aperçu en cache d'un média ;
//! - `preview://localhost/source?path=<chemin>` : miniature d'un fichier de la
//!   source analysée (avant import), lu sans jamais être modifié.
//!
//! Sous Windows, la même adresse s'écrit `http://preview.localhost/…`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{AppHandle, Emitter, Manager, UriSchemeContext, Wry};

use super::AppState;
use crate::db;
use crate::library::media::{cached_preview, full_size_jpeg, generate_previews, missing_previews};
use crate::metadata::media::{format_of, read_metadata};
use crate::raw::preview::{self, SIZES};

pub const SCHEME: &str = "preview";
pub const EVENT_READY: &str = "preview-ready";
/// Taille des miniatures de la source.
const SOURCE_THUMBNAIL: u32 = 256;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Ready {
    media_id: i64,
}

/// File des médias dont il faut produire les aperçus.
pub struct PreviewQueue {
    sender: Mutex<Sender<i64>>,
}

impl PreviewQueue {
    /// Démarre `workers` fils de génération, chacun avec sa connexion.
    pub fn start(app: AppHandle, db_path: PathBuf, cache_root: PathBuf, workers: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<i64>();
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..workers.max(1) {
            let (app, db_path, cache_root, receiver) = (
                app.clone(),
                db_path.clone(),
                cache_root.clone(),
                receiver.clone(),
            );
            std::thread::spawn(move || worker(&app, &db_path, &cache_root, &receiver));
        }
        Self {
            sender: Mutex::new(sender),
        }
    }

    pub fn push(&self, media_id: i64) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(media_id);
        }
    }
}

fn worker(app: &AppHandle, db_path: &Path, cache_root: &Path, receiver: &Mutex<Receiver<i64>>) {
    let Ok(conn) = db::connect(db_path) else {
        return;
    };
    loop {
        let next = match receiver.lock() {
            Ok(rx) => rx.recv(),
            Err(_) => return,
        };
        let Ok(media_id) = next else {
            return;
        };
        match generate_previews(&conn, cache_root, media_id) {
            Ok(_) => {
                let _ = app.emit(EVENT_READY, Ready { media_id });
            }
            Err(error) => eprintln!("aperçu du média {media_id} : {error}"),
        }
    }
}

/// Rattrapage au démarrage : médias importés sans aperçu complet.
pub fn enqueue_missing(app: &AppHandle) {
    let state = app.state::<AppState>();
    let queue = app.state::<PreviewQueue>();
    if let Ok(conn) = state.connect()
        && let Ok(ids) = missing_previews(&conn)
    {
        for id in ids {
            queue.push(id);
        }
    }
}

fn respond(status: StatusCode, content_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        // Les adresses portent un numéro de version côté interface.
        .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn not_found() -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
}

/// Décode `%XX` et `+` d'un paramètre d'adresse.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn query_param(query: &str, name: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .and_then(|(_, value)| percent_decode(value))
}

fn handle(app: &AppHandle, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let state = app.state::<AppState>();
    let uri = request.uri();
    let segments: Vec<&str> = uri.path().split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        ["media", id, "full"] => {
            let Ok(id) = id.parse::<i64>() else {
                return not_found();
            };
            match state.connect().ok().map(|conn| full_size_jpeg(&conn, id)) {
                Some(Ok(Some(bytes))) => respond(StatusCode::OK, "image/jpeg", bytes),
                _ => not_found(),
            }
        }
        ["media", id, size] => {
            let (Ok(id), Ok(size)) = (id.parse::<i64>(), size.parse::<u32>()) else {
                return not_found();
            };
            if !SIZES.contains(&size) {
                return not_found();
            }
            match cached_preview(&state.cache_root, id, size).and_then(|p| std::fs::read(p).ok()) {
                Some(bytes) => respond(StatusCode::OK, "image/jpeg", bytes),
                None => {
                    // Pas encore en cache : on le demande, l'interface sera prévenue.
                    app.state::<PreviewQueue>().push(id);
                    not_found()
                }
            }
        }
        ["source"] => {
            let Some(path) = uri.query().and_then(|q| query_param(q, "path")) else {
                return not_found();
            };
            let path = PathBuf::from(path);
            // Seuls les fichiers de la source analysée peuvent être lus.
            let allowed = state
                .source_root
                .lock()
                .ok()
                .and_then(|root| root.clone())
                .is_some_and(|root| path.starts_with(&root) && path.is_file());
            let Some(format) = format_of(&path).filter(|_| allowed) else {
                return not_found();
            };
            let orientation = read_metadata(&path, format).orientation;
            match preview::thumbnail(&path, format, orientation, SOURCE_THUMBNAIL) {
                Ok(bytes) => respond(StatusCode::OK, "image/jpeg", bytes),
                Err(_) => not_found(),
            }
        }
        _ => not_found(),
    }
}

/// Gestionnaire du protocole : chaque requête est servie dans son propre fil.
pub fn protocol(
    ctx: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    std::thread::spawn(move || responder.respond(handle(&app, &request)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parametres_d_adresse() {
        assert_eq!(
            query_param("path=C%3A%5CDCIM%5C%C3%89t%C3%A9+1.RAF&x=1", "path").as_deref(),
            Some("C:\\DCIM\\Été 1.RAF")
        );
        assert_eq!(query_param("x=1", "path"), None);
        assert_eq!(percent_decode("%ZZ"), None);
    }
}
