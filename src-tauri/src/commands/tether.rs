//! Connexion au boîtier en arrière-plan (décision 0016) : un Fujifilm relié
//! est repéré sans être ouvert ; une fois la séance démarrée, il est ouvert
//! et relevé toutes les demi-secondes. Chaque photo reçue part à l'import
//! par le dossier de réception des boîtiers, dans l'album de la séance.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::error::{CommandError, CommandResult};
use super::watch::WatchService;
use super::{AppState, blocking};
use crate::import::auto::AutoSettings;
use crate::import::watch;
use crate::tether::session::{Session, Update};
use crate::tether::{self, TetherError};

pub const EVENT_STATUS: &str = "tether-status";

/// Relevé du boîtier pendant la séance.
const POLL: Duration = Duration::from_millis(500);
/// Recherche d'un boîtier relié, sans l'ouvrir.
const DETECT: Duration = Duration::from_secs(2);

#[cfg(windows)]
type Link = tether::wpd::WpdTransport;
#[cfg(not(windows))]
type Link = tether::fake::FakeCamera;

/// Boîtier Fujifilm relié : identifiant système et nom.
#[cfg(windows)]
fn detect() -> Option<(String, String)> {
    tether::wpd::devices().ok()?.into_iter().find_map(|d| {
        d.manufacturer
            .eq_ignore_ascii_case("fujifilm")
            .then(|| (d.id, format!("{} {}", d.manufacturer, d.name)))
    })
}

/// Pas encore de connexion au boîtier hors de Windows.
#[cfg(not(windows))]
fn detect() -> Option<(String, String)> {
    None
}

#[cfg(windows)]
fn open(id: &str, reception: &std::path::Path) -> Result<Session<Link>, TetherError> {
    Session::open(
        tether::ptp::Camera::new(tether::wpd::WpdTransport::open(id)?),
        reception,
    )
}

#[cfg(not(windows))]
fn open(_id: &str, _reception: &std::path::Path) -> Result<Session<Link>, TetherError> {
    Err(TetherError::Disconnected)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TetherStatus {
    /// Boîtier relié (« FUJIFILM X-H2 »), séance ou pas.
    pub camera: Option<String>,
    /// Séance démarrée par l'utilisateur.
    pub active: bool,
    /// Boîtier ouvert et relevé.
    pub connected: bool,
    pub library_id: Option<i64>,
    pub album_name: String,
    /// Photos reçues pendant la séance.
    pub received: u32,
    /// Place restante sur la carte (estimation du boîtier).
    pub remaining: Option<u32>,
    /// Photos restées sur la carte après un débranchement, au moins : celles
    /// prises câble débranché ne sont pas comptées.
    pub left_on_card: u32,
    /// Photos illisibles, restées sur la carte.
    pub failed: Vec<String>,
}

enum Message {
    Start { library_id: i64, album_name: String },
    Stop,
}

pub struct TetherService {
    sender: Mutex<Sender<Message>>,
    status: Arc<Mutex<TetherStatus>>,
}

impl TetherService {
    pub fn start(app: AppHandle) -> Self {
        let (sender, receiver) = mpsc::channel::<Message>();
        let status = Arc::new(Mutex::new(TetherStatus::default()));
        let shared = status.clone();
        std::thread::spawn(move || {
            let reception = tether::reception_dir(&app.state::<AppState>().data_dir());
            let mut current = TetherStatus::default();
            let mut session: Option<Session<Link>> = None;
            let mut last_detect: Option<Instant> = None;
            let mut db = None;
            loop {
                let before = current.clone();
                match receiver.recv_timeout(if session.is_some() { POLL } else { DETECT / 4 }) {
                    Ok(Message::Start {
                        library_id,
                        album_name,
                    }) => {
                        current = TetherStatus {
                            camera: current.camera.clone(),
                            active: true,
                            library_id: Some(library_id),
                            album_name,
                            ..TetherStatus::default()
                        };
                        last_detect = None;
                    }
                    Ok(Message::Stop) => {
                        session = None;
                        current.active = false;
                        current.connected = false;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                if session.is_none() && last_detect.is_none_or(|t| t.elapsed() >= DETECT) {
                    last_detect = Some(Instant::now());
                    let found = detect();
                    current.camera = found.as_ref().map(|(_, name)| name.clone());
                    if current.active
                        && let Some((id, _)) = found
                    {
                        match open(&id, &reception) {
                            Ok(s) => {
                                session = Some(s);
                                current.connected = true;
                            }
                            Err(e) => eprintln!("afterdusk : ouverture du boîtier : {e}"),
                        }
                    }
                }
                if let Some(s) = session.as_mut() {
                    if db.is_none() {
                        db = app.state::<AppState>().connect().ok();
                    }
                    let polled = match db.as_ref() {
                        Some(conn) => s.poll(conn),
                        None => Err(TetherError::Device("base inaccessible".to_owned())),
                    };
                    match polled {
                        Ok(updates) => {
                            for update in updates {
                                match update {
                                    Update::Received { path, .. } => {
                                        current.received += 1;
                                        app.state::<WatchService>().received(path);
                                    }
                                    Update::Remaining { shots } => current.remaining = Some(shots),
                                    Update::Failed { name, error } => {
                                        eprintln!("afterdusk : {name} illisible : {error}");
                                        current.failed.push(name);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("afterdusk : connexion au boîtier perdue : {e}");
                            current.left_on_card += s.waiting() as u32;
                            current.connected = false;
                            session = None;
                            last_detect = None;
                        }
                    }
                }
                if current != before {
                    if let Ok(mut s) = shared.lock() {
                        *s = current.clone();
                    }
                    let _ = app.emit(EVENT_STATUS, &current);
                }
            }
        });
        Self {
            sender: Mutex::new(sender),
            status,
        }
    }

    fn send(&self, message: Message) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(message);
        }
    }

    pub fn status(&self) -> TetherStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

#[tauri::command]
pub fn tether_status(service: State<'_, TetherService>) -> TetherStatus {
    service.status()
}

/// Démarre la séance : les photos du boîtier relié iront dans l'album
/// `album_name` de la bibliothèque, avec les réglages d'import donnés.
#[tauri::command]
pub async fn tether_start(
    state: State<'_, AppState>,
    service: State<'_, TetherService>,
    watch_service: State<'_, WatchService>,
    library_id: i64,
    album_name: String,
    settings: AutoSettings,
) -> CommandResult<()> {
    let album_name = album_name.trim().to_owned();
    if album_name.is_empty() {
        return Err(CommandError::new("albumName", "nom d'album vide"));
    }
    let conn = state.connect()?;
    let reception = tether::reception_dir(&state.data_dir());
    let album = album_name.clone();
    let pending = blocking(move || {
        watch::ensure_reception(&conn, &reception, library_id, &album, &settings, true)?;
        // Copies d'une séance précédente déjà dans la bibliothèque.
        let leftovers: Vec<PathBuf> = walkdir::WalkDir::new(&reception)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .collect();
        watch::remove_imported(&conn, library_id, &leftovers)?;
        Ok(leftovers
            .into_iter()
            .filter(|p| p.exists())
            .collect::<Vec<_>>())
    })
    .await?;
    watch_service.reload();
    // Reçues mais pas importées (Afterdusk fermé entre-temps) : importées
    // maintenant dans l'album de la séance.
    for path in pending {
        watch_service.received(path);
    }
    service.send(Message::Start {
        library_id,
        album_name,
    });
    Ok(())
}

/// Arrête la séance ; les photos déjà reçues finissent leur import.
#[tauri::command]
pub fn tether_stop(service: State<'_, TetherService>) {
    service.send(Message::Stop);
}
