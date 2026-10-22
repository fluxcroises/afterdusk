//! Surveillance des dossiers en arrière-plan : les événements du système
//! (notify) signalent les fichiers ; une relecture lente rattrape ceux qu'un
//! partage réseau aurait tus. Un fichier complet part à l'import dans son
//! album, et l'interface reçoit les photos ajoutées.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager, State};

use super::error::CommandResult;
use super::import::{release_slot, take_slot};
use super::previews::PreviewQueue;
use super::{AppState, blocking};
use crate::import::auto::AutoSettings;
use crate::import::watch::{self, Tracker, WatchedFolder};

pub const EVENT_IMPORTED: &str = "watch-imported";

/// Pas de la boucle : stabilité des fichiers et arrivées.
const TICK: Duration = Duration::from_millis(250);
/// Relecture complète des dossiers, en filet de sécurité.
const RESCAN: Duration = Duration::from_secs(30);

enum Message {
    /// Liste des dossiers changée (ajout, retrait, pause).
    Reload,
    /// Fichier signalé par le système.
    Path(PathBuf),
    /// Fichier reçu en entier par FTP : importé sans attendre.
    Received(PathBuf),
}

/// Fil de surveillance, démarré avec l'application.
pub struct WatchService {
    sender: Mutex<Sender<Message>>,
}

impl WatchService {
    /// `reception` : dossier de réception FTP, importé à la fin de chaque
    /// envoi plutôt que d'après la taille (un envoi Wi-Fi peut marquer des
    /// pauses).
    pub fn start(app: AppHandle, reception: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel::<Message>();
        let events = sender.clone();
        std::thread::spawn(move || {
            let mut folders: Vec<WatchedFolder> = Vec::new();
            let mut watcher: Option<notify::RecommendedWatcher> = None;
            let mut tracker = Tracker::default();
            let mut last_scan = Instant::now();
            let mut reload = true;
            loop {
                if reload {
                    reload = false;
                    folders = app
                        .state::<AppState>()
                        .connect()
                        .ok()
                        .and_then(|c| watch::list(&c).ok())
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|f| f.enabled)
                        .collect();
                    let watched: Vec<WatchedFolder> = folders
                        .iter()
                        .filter(|f| f.path != reception)
                        .cloned()
                        .collect();
                    watcher = start_watcher(&watched, events.clone());
                    // Arrivées pendant que l'application était fermée.
                    let now = Instant::now();
                    for folder in &watched {
                        for path in watch::arrivals(folder) {
                            tracker.note(path, now);
                        }
                    }
                    last_scan = now;
                }
                match receiver.recv_timeout(TICK) {
                    Ok(Message::Reload) => reload = true,
                    Ok(Message::Received(path)) => {
                        import_ready(&app, &folders, vec![path], &mut tracker);
                    }
                    Ok(Message::Path(path)) => {
                        if let Some(folder) = folders
                            .iter()
                            .find(|f| f.path != reception && path.starts_with(&f.path))
                            && watch::arrived(folder, &path)
                        {
                            tracker.note(path, Instant::now());
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                let now = Instant::now();
                if now.duration_since(last_scan) >= RESCAN {
                    last_scan = now;
                    for folder in folders.iter().filter(|f| f.path != reception) {
                        for path in watch::arrivals(folder) {
                            tracker.note(path, now);
                        }
                    }
                }
                let ready = tracker.ready(now);
                if !ready.is_empty() {
                    import_ready(&app, &folders, ready, &mut tracker);
                }
                // Garde le surveillant en vie tant que la liste ne change pas.
                let _ = &watcher;
            }
        });
        Self {
            sender: Mutex::new(sender),
        }
    }

    pub fn reload(&self) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(Message::Reload);
        }
    }

    /// Fichier reçu en entier dans le dossier de réception FTP.
    pub fn received(&self, path: PathBuf) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(Message::Received(path));
        }
    }
}

fn start_watcher(
    folders: &[WatchedFolder],
    events: Sender<Message>,
) -> Option<notify::RecommendedWatcher> {
    if folders.is_empty() {
        return None;
    }
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            for path in event.paths {
                let _ = events.send(Message::Path(path));
            }
        }
    })
    .ok()?;
    for folder in folders {
        // Un dossier débranché (disque, NAS) reste couvert par la relecture.
        let _ = watcher.watch(&folder.path, RecursiveMode::Recursive);
    }
    Some(watcher)
}

/// Importe les fichiers prêts, dossier par dossier ; si un import est déjà
/// en cours, ils attendent le passage suivant.
fn import_ready(
    app: &AppHandle,
    folders: &[WatchedFolder],
    ready: Vec<PathBuf>,
    tracker: &mut Tracker,
) {
    let state = app.state::<AppState>();
    let now = Instant::now();
    for folder in folders {
        let files: Vec<PathBuf> = ready
            .iter()
            .filter(|p| p.starts_with(&folder.path))
            .cloned()
            .collect();
        if files.is_empty() {
            continue;
        }
        let Ok(cancel) = take_slot(&state) else {
            for path in files {
                tracker.retry(path, now);
            }
            continue;
        };
        let queue = app.state::<PreviewQueue>();
        let result = state.connect().ok().map(|mut conn| {
            watch::import_files(&mut conn, folder, files, &cancel, |p| {
                // Aperçu en priorité : la photo s'affiche dans le tri au plus vite.
                if let Some(id) = p.media_id {
                    queue.push(id);
                }
            })
        });
        release_slot(app);
        if let Some(Ok(Some(imported))) = result {
            let _ = app.emit(EVENT_IMPORTED, imported);
        }
    }
}

#[tauri::command]
pub async fn watch_folders(state: State<'_, AppState>) -> CommandResult<Vec<WatchedFolder>> {
    let conn = state.connect()?;
    // Le dossier de réception FTP se règle avec la réception, pas ici.
    let reception = crate::ftp::reception_dir(&state.data_dir());
    blocking(move || {
        Ok(watch::list(&conn)?
            .into_iter()
            .filter(|f| f.path != reception)
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn watch_folder_add(
    state: State<'_, AppState>,
    service: State<'_, WatchService>,
    path: PathBuf,
    library_id: i64,
    album_name: String,
    settings: AutoSettings,
) -> CommandResult<i64> {
    let conn = state.connect()?;
    let id = blocking(move || {
        Ok(watch::add(
            &conn,
            &path,
            library_id,
            &album_name,
            &settings,
        )?)
    })
    .await?;
    service.reload();
    Ok(id)
}

#[tauri::command]
pub async fn watch_folder_update(
    state: State<'_, AppState>,
    service: State<'_, WatchService>,
    id: i64,
    album_name: String,
    settings: AutoSettings,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(watch::update(&conn, id, &album_name, &settings)?)).await?;
    service.reload();
    Ok(())
}

#[tauri::command]
pub async fn watch_folder_set_enabled(
    state: State<'_, AppState>,
    service: State<'_, WatchService>,
    id: i64,
    enabled: bool,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(watch::set_enabled(&conn, id, enabled)?)).await?;
    service.reload();
    Ok(())
}

#[tauri::command]
pub async fn watch_folder_delete(
    state: State<'_, AppState>,
    service: State<'_, WatchService>,
    id: i64,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(watch::delete(&conn, id)?)).await?;
    service.reload();
    Ok(())
}
