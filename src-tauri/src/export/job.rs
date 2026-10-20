//! Export d'une sélection avec un ou plusieurs presets, en arrière-plan :
//! plusieurs photos à la fois, progression rapportée après chaque fichier,
//! arrêt possible entre deux fichiers. Les originaux ne sont qu'ouverts en
//! lecture ; une copie d'original est vérifiée par somme de contrôle.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::ExportError;
use super::color::ColorSpace;
use super::encode;
use super::metadata::{self, PhotoMeta};
use super::presets::{self, ExportPreset, ExportSettings, Output};
use super::render::{RenderOptions, Resize, render_photo};
use crate::db;
use crate::develop::lens::{self, Shot};
use crate::develop::store::{self, ORIGINAL};
use crate::import::copy::copy_verified;
use crate::library::{get_library, paths};
use crate::metadata::media::{MediaFormat, format_of, read_metadata};
use crate::metadata::xmp::read_sidecar;
use crate::raw::RawFile;

/// Un preset de l'export et son sous-dossier (plusieurs presets : un
/// sous-dossier chacun, nommé par l'interface).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub preset: String,
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub media_ids: Vec<i64>,
    pub targets: Vec<Target>,
    pub destination: PathBuf,
}

/// Échec d'un fichier ; l'export continue avec les autres.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub file: String,
    pub preset: String,
    /// Code traduisible (`unsupported`, `offline`, `internal`…).
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub current: Option<String>,
    pub failures: Vec<Failure>,
}

/// Une photo de la bibliothèque, telle que l'export la voit.
struct Photo {
    path: PathBuf,
    sidecar: Option<PathBuf>,
    name: String,
    original: String,
    captured_at: Option<String>,
}

/// Échec avant tout export : fichier, code, détail.
type PhotoError = (String, String, String);

/// Ligne `media` : bibliothèque, chemin, XMP, nom d'origine, date.
type MediaRow = (i64, String, Option<String>, String, Option<String>);

fn photo(conn: &Connection, id: i64) -> Result<Photo, PhotoError> {
    let row: Option<MediaRow> = conn
        .query_row(
            "SELECT library_id, path, sidecar_path, original_name, captured_at FROM media WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(|e| (id.to_string(), "internal".into(), e.to_string()))?;
    let Some((library_id, rel, sidecar, original, captured_at)) = row else {
        return Err((id.to_string(), "mediaNotFound".into(), String::new()));
    };
    let name = Path::new(&rel)
        .file_name()
        .map_or_else(|| rel.clone(), |n| n.to_string_lossy().into_owned());
    let fail = |code: &str, e: String| (name.clone(), code.to_owned(), e);
    let lib = get_library(conn, library_id).map_err(|e| fail("internal", e.to_string()))?;
    if !lib.is_online {
        return Err(fail("libraryOffline", lib.root_path.display().to_string()));
    }
    let path =
        paths::to_absolute(&lib.root_path, &rel).map_err(|e| fail("internal", e.to_string()))?;
    let sidecar = sidecar.and_then(|s| paths::to_absolute(&lib.root_path, &s).ok());
    Ok(Photo {
        path,
        sidecar,
        name,
        original,
        captured_at,
    })
}

/// Exporte une photo avec un preset ; renvoie le fichier écrit.
fn export_one(
    conn: &Connection,
    id: i64,
    photo: &Photo,
    preset: &ExportPreset,
    settings: &ExportSettings,
    dir: &Path,
    seq: usize,
) -> Result<PathBuf, ExportError> {
    let stem = presets::file_stem(
        &preset.naming,
        &photo.name,
        &photo.original,
        photo.captured_at.as_deref(),
        seq,
    );
    std::fs::create_dir_all(dir)?;
    let Some(format) = preset.output.format() else {
        // Copie vérifiée de l'original, et de son XMP.
        let ext = photo
            .path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        let target = presets::free_path(dir, &stem, &ext);
        copy_verified(&photo.path, &target).map_err(|e| ExportError::Copy(e.to_string()))?;
        if let Some(side) = photo.sidecar.as_ref().filter(|p| p.exists()) {
            let xmp = target.with_extension("xmp");
            if !xmp.exists() {
                copy_verified(side, &xmp).map_err(|e| ExportError::Copy(e.to_string()))?;
            }
        }
        return Ok(target);
    };
    if format_of(&photo.path) != Some(MediaFormat::Raw) {
        return Err(ExportError::Unsupported(photo.name.clone()));
    }
    let develop =
        store::load(conn, id, ORIGINAL).map_err(|e| ExportError::Settings(e.to_string()))?;
    let mut raw = RawFile::open(&photo.path)?;
    let profile = lens::find_profile(&Shot::from(&raw.info()));
    let color = if matches!(preset.output, Output::Avif { .. }) {
        ColorSpace::Srgb
    } else {
        preset.color
    };
    let options = RenderOptions {
        resize: preset.resize,
        sharpen: preset.sharpen,
        color,
    };
    let mark = settings.watermark.as_ref().filter(|_| preset.watermark);
    let image = render_photo(&mut raw, &develop, profile.as_ref(), options, mark)?;
    drop(raw);
    let meta = PhotoMeta {
        camera: read_metadata(&photo.path, MediaFormat::Raw),
        sidecar: photo
            .sidecar
            .as_ref()
            .filter(|p| p.exists())
            .and_then(|p| read_sidecar(p).ok().flatten()),
    };
    let embedded = metadata::embedded(&meta, preset.metadata)?;
    let target = presets::free_path(dir, &stem, format.extension());
    encode::write(&image, format, &color.icc()?, &embedded, &target)?;
    Ok(target)
}

/// Plusieurs photos à la fois, moins pour les grandes sorties (mémoire).
fn workers(presets: &[&ExportPreset]) -> usize {
    let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
    let large = presets.iter().any(|p| {
        p.output.format().is_some() && matches!(p.resize, Resize::Full | Resize::Percent(_))
    });
    let max = if large { 2 } else { 4 };
    (cores / 2).clamp(1, max)
}

fn code_of(e: &ExportError) -> &'static str {
    match e {
        ExportError::Unsupported(_) => "developUnsupported",
        ExportError::Copy(_) => "copyFailed",
        ExportError::Io(_) => "writeFailed",
        _ => "internal",
    }
}

/// Lance l'export ; `report` reçoit la progression après chaque fichier.
/// Renvoie la progression finale.
pub fn run(
    db_path: &Path,
    settings: &ExportSettings,
    request: &ExportRequest,
    cancel: &AtomicBool,
    report: &(dyn Fn(&Progress) + Sync),
) -> Result<Progress, ExportError> {
    let targets: Vec<(&ExportPreset, PathBuf)> = request
        .targets
        .iter()
        .filter_map(|t| {
            let preset = settings.presets.iter().find(|p| p.id == t.preset)?;
            let dir = match (&t.folder, request.targets.len() > 1) {
                (Some(f), true) => request
                    .destination
                    .join(presets::file_stem(f, f, f, None, 0)),
                _ => request.destination.clone(),
            };
            Some((preset, dir))
        })
        .collect();
    if targets.is_empty() {
        return Err(ExportError::Settings("aucun preset".into()));
    }
    let total = request.media_ids.len() * targets.len();
    let next = AtomicUsize::new(0);
    let state = Mutex::new(Progress {
        done: 0,
        total,
        current: None,
        failures: Vec::new(),
    });
    let presets_only: Vec<&ExportPreset> = targets.iter().map(|(p, _)| *p).collect();
    std::thread::scope(|s| {
        for _ in 0..workers(&presets_only) {
            s.spawn(|| {
                let Ok(conn) = db::connect(db_path) else {
                    return;
                };
                loop {
                    if cancel.load(Ordering::SeqCst) {
                        return;
                    }
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(&id) = request.media_ids.get(index) else {
                        return;
                    };
                    let mut failures = Vec::new();
                    let mut name = id.to_string();
                    match photo(&conn, id) {
                        Ok(p) => {
                            name.clone_from(&p.name);
                            for (preset, dir) in &targets {
                                if cancel.load(Ordering::SeqCst) {
                                    break;
                                }
                                if let Err(e) =
                                    export_one(&conn, id, &p, preset, settings, dir, index + 1)
                                {
                                    failures.push(Failure {
                                        file: p.name.clone(),
                                        preset: preset.id.clone(),
                                        code: code_of(&e).into(),
                                        detail: e.to_string(),
                                    });
                                }
                            }
                        }
                        Err((file, code, detail)) => {
                            for (preset, _) in &targets {
                                failures.push(Failure {
                                    file: file.clone(),
                                    preset: preset.id.clone(),
                                    code: code.clone(),
                                    detail: detail.clone(),
                                });
                            }
                        }
                    }
                    let snapshot = {
                        let Ok(mut st) = state.lock() else {
                            return;
                        };
                        st.done += targets.len();
                        st.current = Some(name);
                        st.failures.extend(failures);
                        st.clone()
                    };
                    report(&snapshot);
                }
            });
        }
    });
    let mut done = state
        .into_inner()
        .map_err(|e| ExportError::Settings(e.to_string()))?;
    done.current = None;
    Ok(done)
}
