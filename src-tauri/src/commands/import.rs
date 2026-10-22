//! Import depuis l'interface : analyse de la source, lancement en tâche de
//! fond avec progression par événements, arrêt et reprise.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::error::{CommandError, CommandResult};
use super::previews::PreviewQueue;
use super::{AppState, blocking};
use crate::import::{
    self, ImportOptions, ImportOutcome, RecentImport, ResumableImport, auto, scan,
};
use crate::library::pairs;
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
    /// Paire RAW + JPEG : le JPEG (ou HEIF) du boîtier, importé avec le RAW.
    pub companions: Vec<CompanionEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionEntry {
    pub path: PathBuf,
    pub file_name: String,
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
        // Paires RAW + JPEG : une case par déclenchement.
        let candidates: Vec<pairs::Candidate<'_>> = result
            .files
            .iter()
            .map(|f| pairs::Candidate {
                path: &f.path,
                format: f.format,
                captured_at: Some(f.captured_at.as_str()),
            })
            .collect();
        let mut companions: HashMap<usize, Vec<CompanionEntry>> = HashMap::new();
        let mut is_companion = vec![false; result.files.len()];
        for (raw, companion) in pairs::find_pairs(&candidates) {
            let f = &result.files[companion];
            is_companion[companion] = true;
            companions.entry(raw).or_default().push(CompanionEntry {
                path: f.path.clone(),
                file_name: f.file_name.clone(),
                already_imported: known.contains(&f.fingerprint),
            });
        }
        let mut days: Vec<SourceDay> = Vec::new();
        for (index, file) in result.files.into_iter().enumerate() {
            if is_companion[index] {
                continue;
            }
            let companions = companions.remove(&index).unwrap_or_default();
            let entry = SourceEntry {
                // Déjà importée : tous les fichiers du déclenchement le sont.
                already_imported: known.contains(&file.fingerprint)
                    && companions.iter().all(|c| c.already_imported),
                companions,
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

/// Boîtiers à numéro de série d'un import, proposés pour l'import automatique.
#[tauri::command]
pub async fn auto_import_candidates(
    state: State<'_, AppState>,
    import_id: i64,
) -> CommandResult<Vec<auto::Body>> {
    let conn = state.connect()?;
    blocking(move || Ok(auto::candidates(&conn, import_id)?)).await
}

/// Les prochaines cartes de ces boîtiers s'importeront seules, avec ces réglages.
#[tauri::command]
pub async fn auto_import_rule_add(
    state: State<'_, AppState>,
    import_id: i64,
    settings: auto::AutoSettings,
) -> CommandResult<usize> {
    let conn = state.connect()?;
    blocking(move || Ok(auto::add_from_import(&conn, import_id, &settings)?)).await
}

#[tauri::command]
pub async fn auto_import_rules(state: State<'_, AppState>) -> CommandResult<Vec<auto::Rule>> {
    let conn = state.connect()?;
    blocking(move || Ok(auto::list(&conn)?)).await
}

#[tauri::command]
pub async fn auto_import_rule_delete(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(auto::delete(&conn, id)?)).await
}

/// Issue de l'insertion d'une carte, pour l'interface.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AutoCard {
    /// Aucun boîtier connu : comportement habituel.
    Unknown,
    /// Un import est déjà en cours : comportement habituel.
    Busy,
    NothingNew {
        camera: String,
    },
    Started {
        camera: String,
        import_id: i64,
        files: usize,
    },
}

/// Carte insérée : si elle vient d'un boîtier connu, l'import part seul.
/// La carte n'est que lue ; l'import garde toutes ses garanties.
#[tauri::command]
pub async fn auto_import_card(
    app: AppHandle,
    state: State<'_, AppState>,
    root: PathBuf,
    source_label: String,
) -> CommandResult<AutoCard> {
    let conn = state.connect()?;
    let card = blocking(move || Ok(auto::plan_for_card(&conn, &root, &source_label)?)).await?;
    let (camera, rule_id, options) = match card {
        auto::CardPlan::Unknown => return Ok(AutoCard::Unknown),
        auto::CardPlan::NothingNew { camera } => return Ok(AutoCard::NothingNew { camera }),
        auto::CardPlan::Import {
            camera,
            rule_id,
            options,
        } => (camera, rule_id, options),
    };
    let Ok(cancel) = take_slot(&state) else {
        return Ok(AutoCard::Busy);
    };
    let mut conn = match state.connect() {
        Ok(conn) => conn,
        Err(e) => {
            release_slot(&app);
            return Err(e);
        }
    };
    let files = options.files.len();
    let planned = blocking(move || {
        let id = import::plan(&mut conn, &options)?;
        auto::touch(&conn, rule_id)?;
        Ok(id)
    })
    .await;
    match planned {
        Ok(import_id) => {
            spawn_run(app, import_id, cancel);
            Ok(AutoCard::Started {
                camera,
                import_id,
                files,
            })
        }
        Err(e) => {
            release_slot(&app);
            Err(e)
        }
    }
}
