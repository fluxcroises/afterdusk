//! Réception FTP : démarrage et arrêt du serveur selon le réglage, état
//! affiché dans les réglages, certificat, et vidage de la réception.

use std::net::{IpAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::error::{CommandError, CommandResult};
use super::watch::WatchService;
use super::{AppState, blocking};
use crate::ftp::{self, FtpSettings};
use crate::import::auto::AutoSettings;
use crate::import::{known_fingerprints, scan, watch};
use crate::metadata::media::format_of;

/// État du serveur, pour les réglages.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FtpStatus {
    pub running: bool,
    /// Pourquoi le serveur ne tourne pas (port occupé…).
    pub error: Option<String>,
}

#[derive(Default)]
pub struct FtpService {
    stop: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    status: Arc<Mutex<FtpStatus>>,
    /// Numéro du dernier démarrage : un serveur arrêté n'écrase pas l'état du suivant.
    generation: Arc<AtomicU64>,
}

impl FtpService {
    /// Arrête le serveur s'il tourne, puis le relance si la réception est active.
    pub fn restart(&self, app: &AppHandle) {
        if let Ok(mut stop) = self.stop.lock()
            && let Some(sender) = stop.take()
        {
            let _ = sender.send(());
        }
        let status = self.status.clone();
        let set_status = move |s: FtpStatus| {
            if let Ok(mut current) = status.lock() {
                *current = s;
            }
        };
        let state = app.state::<AppState>();
        let data_dir = state.data_dir();
        let reception = ftp::reception_dir(&data_dir);
        let Ok(conn) = state.connect() else {
            return;
        };
        let Ok(settings) = ftp::get(&conn) else {
            return;
        };
        // Le dossier de réception suit le réglage : surveillé quand la réception l'est.
        if let Some(library_id) = settings.library_id {
            let _ = watch::ensure_reception(
                &conn,
                &reception,
                library_id,
                &settings.album_name,
                &AutoSettings {
                    backup: true,
                    ..Default::default()
                },
                settings.enabled,
            );
            app.state::<WatchService>().reload();
        }
        if !settings.enabled || settings.library_id.is_none() {
            self.generation.fetch_add(1, Ordering::SeqCst);
            set_status(FtpStatus::default());
            return;
        }
        let cert = match ftp::certificate(&data_dir) {
            Ok(cert) => cert,
            Err(e) => {
                set_status(FtpStatus {
                    running: false,
                    error: Some(e.to_string()),
                });
                return;
            }
        };
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        let handle = app.clone();
        let on_file: Arc<dyn Fn(PathBuf) + Send + Sync> =
            Arc::new(move |path| handle.state::<WatchService>().received(path));
        let server = ftp::server(&settings, &reception, &cert, on_file, async move {
            let _ = receiver.await;
        });
        let server = match server {
            Ok(server) => server,
            Err(e) => {
                set_status(FtpStatus {
                    running: false,
                    error: Some(e.to_string()),
                });
                return;
            }
        };
        if let Ok(mut stop) = self.stop.lock() {
            *stop = Some(sender);
        }
        set_status(FtpStatus {
            running: true,
            error: None,
        });
        let port = settings.port;
        let status = self.status.clone();
        let generation = self.generation.clone();
        let mine = generation.fetch_add(1, Ordering::SeqCst) + 1;
        tauri::async_runtime::spawn(async move {
            // Toutes les interfaces : l'authentification refuse ce qui n'est pas local.
            let result = server.listen(format!("0.0.0.0:{port}")).await;
            if generation.load(Ordering::SeqCst) != mine {
                return;
            }
            if let Ok(mut current) = status.lock() {
                *current = FtpStatus {
                    running: false,
                    error: result.err().map(|e| e.to_string()),
                };
            }
        });
    }

    fn status(&self) -> FtpStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

/// Adresse de l'ordinateur sur le réseau local, à saisir dans le boîtier.
/// Le système choisit l'interface d'une route ; aucun paquet n'est envoyé.
fn local_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.168.255.255:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && ftp::is_local(ip)).then_some(ip)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FtpInfo {
    pub settings: FtpSettings,
    pub status: FtpStatus,
    pub address: Option<String>,
    pub reception: PathBuf,
    /// Empreinte du certificat FTPS, une fois créé.
    pub fingerprint: Option<String>,
}

#[tauri::command]
pub async fn ftp_get(
    state: State<'_, AppState>,
    service: State<'_, FtpService>,
) -> CommandResult<FtpInfo> {
    let conn = state.connect()?;
    let data_dir = state.data_dir();
    let status = service.status();
    blocking(move || {
        let settings = ftp::get(&conn).map_err(|e| CommandError::new("ftp", e))?;
        let fingerprint = settings
            .enabled
            .then(|| ftp::certificate(&data_dir).ok().map(|c| c.fingerprint))
            .flatten();
        Ok(FtpInfo {
            settings,
            status,
            address: local_address().map(|ip| ip.to_string()),
            reception: ftp::reception_dir(&data_dir),
            fingerprint,
        })
    })
    .await
}

#[tauri::command]
pub async fn ftp_set(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: FtpSettings,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || ftp::set(&conn, &settings).map_err(|e| CommandError::new("ftp", e))).await?;
    let handle = app.clone();
    blocking(move || {
        handle.state::<FtpService>().restart(&handle);
        Ok(())
    })
    .await
}

/// Enregistre le certificat (DER) là où l'utilisateur le demande, pour le
/// copier sur une carte et l'importer dans le boîtier.
#[tauri::command]
pub async fn ftp_save_certificate(state: State<'_, AppState>, path: PathBuf) -> CommandResult<()> {
    let data_dir = state.data_dir();
    blocking(move || {
        let cert = ftp::certificate(&data_dir).map_err(|e| CommandError::new("ftp", e))?;
        std::fs::copy(&cert.der, &path).map_err(|e| CommandError::new("ftp", e))?;
        Ok(())
    })
    .await
}

/// Vide la réception : seuls les fichiers déjà dans la bibliothèque sont
/// supprimés ; renvoie le nombre de fichiers supprimés et gardés.
#[tauri::command]
pub async fn ftp_clear_reception(state: State<'_, AppState>) -> CommandResult<(usize, usize)> {
    let conn = state.connect()?;
    let data_dir = state.data_dir();
    blocking(move || {
        let settings = ftp::get(&conn).map_err(|e| CommandError::new("ftp", e))?;
        let Some(library_id) = settings.library_id else {
            return Ok((0, 0));
        };
        let known = known_fingerprints(&conn, library_id)?;
        let reception = ftp::reception_dir(&data_dir);
        let (mut removed, mut kept) = (0, 0);
        for entry in walkdir::WalkDir::new(&reception)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let imported = format_of(path)
                .and_then(|f| scan::describe(path, f).ok())
                .is_some_and(|d| known.contains(&d.fingerprint));
            if imported && std::fs::remove_file(path).is_ok() {
                removed += 1;
            } else {
                kept += 1;
            }
        }
        Ok((removed, kept))
    })
    .await
}
