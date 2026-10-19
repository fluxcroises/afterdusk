//! Retoucher : ouverture d'une photo, rendu à chaque réglage, enregistrement.
//!
//! L'original est décodé une fois à l'ouverture ; l'image de travail reste en
//! mémoire (et sur le processeur graphique s'il y en a un) tant que la photo
//! est ouverte, si bien qu'un réglage ne coûte qu'un rendu.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};

use super::error::{CommandError, CommandResult};
use super::marks::XmpWriter;
use super::{AppState, blocking};
use crate::develop::gpu::{GpuImage, GpuRenderer};
use crate::develop::pipeline::{Clipping, RenderParams, WorkingImage, render};
use crate::develop::settings::{DevelopSettings, WhiteBalance};
use crate::develop::source::{self, PREVIEW_SIDE, SourceError};
use crate::develop::store::{self, StoreError};
use crate::develop::white;

/// Variable d'environnement : `AFTERDUSK_GPU=0` force le rendu processeur.
const GPU_ENV: &str = "AFTERDUSK_GPU";

/// En-tête du rendu renvoyé à l'interface : largeur, hauteur, durée du rendu
/// en microsecondes, moteur (1 = GPU), en u32 LE ; suivent l'histogramme
/// (4 × 256 u32 : rouge, vert, bleu, luminance) puis les pixels RGBA.
const HEADER: usize = 16;

impl From<SourceError> for CommandError {
    fn from(e: SourceError) -> Self {
        let code = match &e {
            SourceError::NotFound(_) => "mediaNotFound",
            SourceError::Offline(_) => "libraryOffline",
            SourceError::Unsupported(_) => "developUnsupported",
            _ => "internal",
        };
        Self::new(code, e)
    }
}

impl From<StoreError> for CommandError {
    fn from(e: StoreError) -> Self {
        Self::new("internal", e)
    }
}

struct Session {
    image: WorkingImage,
    gpu: Option<GpuImage>,
}

/// Moteur de rendu : processeur graphique (choisi au premier usage) et photo ouverte.
pub struct DevelopEngine {
    gpu: OnceLock<Option<GpuRenderer>>,
    session: Mutex<Option<Session>>,
}

impl Default for DevelopEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DevelopEngine {
    pub fn new() -> Self {
        Self {
            gpu: OnceLock::new(),
            session: Mutex::new(None),
        }
    }

    fn gpu(&self) -> Option<&GpuRenderer> {
        self.gpu
            .get_or_init(|| {
                if std::env::var_os(GPU_ENV).is_some_and(|v| v == "0") {
                    return None;
                }
                GpuRenderer::new()
                    .inspect_err(|e| eprintln!("rendu au processeur : {e}"))
                    .ok()
            })
            .as_ref()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopOpened {
    pub media_id: i64,
    pub width: u32,
    pub height: u32,
    pub settings: DevelopSettings,
    /// Balance du boîtier, en kelvins et en teinte, pour les curseurs.
    pub as_shot: AsShot,
    /// Processeur graphique utilisé, ou rien pour le rendu processeur.
    pub adapter: Option<String>,
    pub decode_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsShot {
    pub temperature: f64,
    pub tint: f64,
}

/// Ouvre une photo dans le Retoucher : décode l'original et charge ses réglages.
#[tauri::command]
pub async fn develop_open(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: i64,
) -> CommandResult<DevelopOpened> {
    let conn = state.connect()?;
    blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let started = Instant::now();
        let image = source::working_image(&conn, media_id, PREVIEW_SIDE)?;
        let decode_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let settings = store::load(&conn, media_id)?;
        let renderer = engine.gpu();
        let gpu = renderer.and_then(|r| {
            r.upload(&image)
                .inspect_err(|e| eprintln!("rendu au processeur : {e}"))
                .ok()
        });
        let (temperature, tint) = white::temperature_of(white::xy_of(image.as_shot_white));
        let opened = DevelopOpened {
            media_id,
            width: image.width,
            height: image.height,
            settings,
            as_shot: AsShot {
                temperature: temperature.round(),
                tint: tint.round(),
            },
            adapter: gpu
                .as_ref()
                .and(renderer)
                .map(|r| r.adapter_name().to_owned()),
            decode_ms,
        };
        let mut session = engine
            .session
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        *session = Some(Session { image, gpu });
        Ok(opened)
    })
    .await
}

/// Rend la photo ouverte avec ces réglages : en-tête, histogramme, pixels.
#[tauri::command]
pub async fn develop_render(
    app: AppHandle,
    settings: DevelopSettings,
    clipping: Option<Clipping>,
) -> CommandResult<Response> {
    let bytes = blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let session = engine
            .session
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        let session = session
            .as_ref()
            .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?;
        let params = RenderParams::from_settings(
            &settings.sanitized(),
            session.image.as_shot_white,
            clipping.unwrap_or_default(),
        );
        let started = Instant::now();
        let gpu = engine.gpu().zip(session.gpu.as_ref()).and_then(|(r, img)| {
            r.render(img, &params)
                .inspect_err(|e| eprintln!("rendu au processeur : {e}"))
                .ok()
        });
        let on_gpu = gpu.is_some();
        let rendered = gpu.unwrap_or_else(|| render(&session.image, &params));
        let micros = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        let mut out =
            Vec::with_capacity(HEADER + rendered.histogram.len() * 4 + rendered.rgba.len());
        for v in [
            session.image.width,
            session.image.height,
            micros,
            u32::from(on_gpu),
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in &rendered.histogram {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&rendered.rgba);
        Ok(out)
    })
    .await?;
    Ok(Response::new(bytes))
}

/// Pipette : balance des blancs qui rend neutre le point (x, y) de l'image
/// affichée, en pixels de l'aperçu.
#[tauri::command]
pub fn develop_pick_white(
    engine: State<'_, DevelopEngine>,
    x: u32,
    y: u32,
) -> CommandResult<WhiteBalance> {
    let session = engine
        .session
        .lock()
        .map_err(|e| CommandError::new("internal", e))?;
    let image = &session
        .as_ref()
        .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?
        .image;
    let xyz = image
        .sample_xyz(x, y)
        .ok_or_else(|| CommandError::new("internal", format!("point {x}, {y} hors de l'image")))?;
    Ok(white::neutralizing(xyz, image.as_shot_white))
}

/// Ferme la photo ouverte et libère sa mémoire.
#[tauri::command]
pub fn develop_close(engine: State<'_, DevelopEngine>) {
    if let Ok(mut session) = engine.session.lock() {
        *session = None;
    }
}

/// Enregistre les réglages (base, historique, XMP au passage suivant du fil d'écriture).
#[tauri::command]
pub async fn develop_save(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    settings: DevelopSettings,
    label: String,
) -> CommandResult<DevelopSettings> {
    let mut conn = state.connect()?;
    let saved = blocking(move || Ok(store::save(&mut conn, media_id, settings, &label)?)).await?;
    writer.wake();
    Ok(saved)
}
