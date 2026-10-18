//! Import depuis l'interface : analyse de la source, lancement en tâche de
//! fond avec progression par événements, arrêt et reprise.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::error::{CommandError, CommandResult};
use super::previews::PreviewQueue;
use super::{AppState, blocking};
use crate::import::{self, ImportOptions, ImportOutcome, RecentImport, ResumableImport, scan};
use crate::metadata::media::MediaFormat;

pub const EVENT_PROGRESS: &str = "import-progress";
pub const EVENT_FINISHED: &str = "import-finished";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceEntry {
    pub path: PathBuf,
    pub file_name: String,
    pub size: u64,
    pub format: MediaFormat,
    pub captured_at: String,
    pub camera: String,
    pub already_imported: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDay {
    /// `AAAA-MM-JJ`
    pub day: String,
    pub files: Vec<SourceEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    pub root: PathBuf,
    pub days: Vec<SourceDay>,
    pub ignored: usize,
}

/// Liste les médias de la source, groupés par jour ; ceux déjà présents dans
/// la bibliothèque sont signalés pour être grisés.
#[tauri::command]
pub async fn source_scan(
    state: State<'_, AppState>,
    path: PathBuf,
    library_id: Option<i64>,
) -> CommandResult<SourceView> {
    let conn = state.connect()?;
    if let Ok(mut root) = state.source_root.lock() {
        *root = Some(path.clone());
    }
    blocking(move || {
        let known = match library_id {
            Some(id) => import::known_fingerprints(&conn, id)?,
            None => Default::default(),
        };
        let result = scan::scan(&path)?;
        let mut days: Vec<SourceDay> = Vec::new();
        for file in result.files {
            let entry = SourceEntry {
                already_imported: known.contains(&file.fingerprint),
                camera: file.metadata.camera_label(),
                path: file.path.clone(),
                file_name: file.file_name.clone(),
                size: file.size,
                format: file.format,
                captured_at: file.captured_at.clone(),
            };
            let day = file.day().to_owned();
            match days.last_mut() {
                Some(last) if last.day == day => last.files.push(entry),
                _ => days.push(SourceDay {
                    day,
                    files: vec![entry],
                }),
            }
        }
        Ok(SourceView {
            root: path,
            days,
            ignored: result.ignored,
        })
    })
    .await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Finished {
    import_id: i64,
    outcome: Option<ImportOutcome>,
    error: Option<CommandError>,
}

fn take_slot(state: &AppState) -> CommandResult<Arc<AtomicBool>> {
    let mut slot = state
        .running_import
        .lock()
        .map_err(|e| CommandError::new("internal", e))?;
    if slot.is_some() {
        return Err(CommandError::new(
            "importRunning",
            "un import est déjà en cours",
        ));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    *slot = Some(cancel.clone());
    Ok(cancel)
}

fn release_slot(app: &AppHandle) {
    if let Ok(mut slot) = app.state::<AppState>().running_import.lock() {
        *slot = None;
    }
}

/// Exécute l'import dans un fil dédié ; la progression part en événements.
fn spawn_run(app: AppHandle, import_id: i64, cancel: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let result = app.state::<AppState>().connect().and_then(|mut conn| {
            let queue = app.state::<PreviewQueue>();
            import::run(&mut conn, import_id, &cancel, |progress| {
                // Aperçus dès la fin de la copie de chaque fichier, en parallèle de l'import.
                if let Some(media_id) = progress.media_id {
                    queue.push(media_id);
                }
                let _ = app.emit(EVENT_PROGRESS, progress);
            })
            .map_err(CommandError::from)
        });
        release_slot(&app);
        let (outcome, error) = match result {
            Ok(outcome) => (Some(outcome), None),
            Err(error) => (None, Some(error)),
        };
        let _ = app.emit(
            EVENT_FINISHED,
            Finished {
                import_id,
                outcome,
                error,
            },
        );
    });
}

/// Prépare puis lance un import ; renvoie son identifiant sans attendre la fin.
#[tauri::command]
pub async fn import_start(
    app: AppHandle,
    state: State<'_, AppState>,
    options: ImportOptions,
) -> CommandResult<i64> {
    let cancel = take_slot(&state)?;
    let mut conn = match state.connect() {
        Ok(conn) => conn,
        Err(e) => {
            release_slot(&app);
            return Err(e);
        }
    };
    let planned = blocking(move || Ok(import::plan(&mut conn, &options)?)).await;
    match planned {
        Ok(import_id) => {
            spawn_run(app, import_id, cancel);
            Ok(import_id)
        }
        Err(e) => {
            release_slot(&app);
            Err(e)
        }
    }
}

/// Reprend un import interrompu.
#[tauri::command]
pub fn import_resume(
    app: AppHandle,
    state: State<'_, AppState>,
    import_id: i64,
) -> CommandResult<()> {
    let cancel = take_slot(&state)?;
    spawn_run(app, import_id, cancel);
    Ok(())
}

/// Demande l'arrêt de l'import en cours, entre deux fichiers.
#[tauri::command]
pub fn import_cancel(state: State<'_, AppState>) -> CommandResult<()> {
    if let Ok(slot) = state.running_import.lock()
        && let Some(cancel) = slot.as_ref()
    {
        cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub async fn imports_resumable(state: State<'_, AppState>) -> CommandResult<Vec<ResumableImport>> {
    let conn = state.connect()?;
    blocking(move || Ok(import::resumable_imports(&conn)?)).await
}

/// Activité récente de l'accueil : derniers imports et leurs premières photos.
#[tauri::command]
pub async fn imports_recent(
    state: State<'_, AppState>,
    library_id: i64,
) -> CommandResult<Vec<RecentImport>> {
    let conn = state.connect()?;
    blocking(move || Ok(import::recent_imports(&conn, library_id, 5, 24)?)).await
}
