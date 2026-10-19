//! Surveillance des cartes et boîtiers : un relevé par seconde, en arrière-plan,
//! pour qu'une carte insérée soit proposée en moins de 5 secondes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use super::error::{CommandError, CommandResult};
use super::{AppState, blocking};
use crate::devices::{self, Device, InsertBehavior};

pub const EVENT_ATTACHED: &str = "device-attached";
pub const EVENT_DETACHED: &str = "device-detached";

/// Intervalle entre deux relevés des volumes.
const POLL: Duration = Duration::from_secs(1);

/// Supports présents, tenus à jour par la surveillance.
#[derive(Default, Clone)]
pub struct DeviceWatch {
    current: Arc<Mutex<Vec<Device>>>,
}

impl DeviceWatch {
    /// Relève les supports déjà là (sans les annoncer), puis surveille.
    pub fn start(app: AppHandle) -> Self {
        let watch = Self::default();
        let initial = devices::detect(&devices::candidate_roots());
        if let Ok(mut current) = watch.current.lock() {
            *current = initial;
        }
        let shared = watch.current.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(POLL);
                let now = devices::detect(&devices::candidate_roots());
                let Ok(mut current) = shared.lock() else {
                    return;
                };
                let (added, removed) = devices::diff(&current, &now);
                *current = now;
                drop(current);
                for d in removed {
                    let _ = app.emit(EVENT_DETACHED, d);
                }
                for d in added {
                    let _ = app.emit(EVENT_ATTACHED, d);
                }
            }
        });
        watch
    }
}

#[tauri::command]
pub fn devices_list(watch: State<'_, DeviceWatch>) -> Vec<Device> {
    watch.current.lock().map(|d| d.clone()).unwrap_or_default()
}

#[tauri::command]
pub async fn insert_behavior_get(state: State<'_, AppState>) -> CommandResult<InsertBehavior> {
    let conn = state.connect()?;
    blocking(move || devices::insert_behavior(&conn).map_err(|e| CommandError::new("internal", e)))
        .await
}

#[tauri::command]
pub async fn insert_behavior_set(
    state: State<'_, AppState>,
    behavior: InsertBehavior,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || {
        devices::set_insert_behavior(&conn, behavior).map_err(|e| CommandError::new("internal", e))
    })
    .await
}

/// La fenêtre principale clignote dans la barre des tâches (Dock sur Mac).
#[tauri::command]
pub fn request_attention(app: AppHandle, label: String) {
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.request_user_attention(Some(tauri::UserAttentionType::Informational));
    }
}
