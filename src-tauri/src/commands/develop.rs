//! Retoucher : ouverture d'une photo, rendu à chaque réglage, enregistrement.
//!
//! L'original est décodé une fois à l'ouverture ; l'image de travail reste en
//! mémoire (et sur le processeur graphique s'il y en a un) tant que la photo
//! est ouverte, si bien qu'un réglage ne coûte qu'un rendu.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};

use super::error::{CommandError, CommandResult};
use super::marks::XmpWriter;
use super::{AppState, blocking};
use crate::develop::auto;
use crate::develop::geometry;
use crate::develop::gpu::{GpuImage, GpuRenderer};
use crate::develop::pipeline::{Clipping, RenderParams, WorkingImage, render};
use crate::develop::settings::Geometry;
use crate::develop::settings::{DevelopSettings, WhiteBalance};
use crate::develop::source::{self, PREVIEW_SIDE, SourceError};
use crate::develop::store::{self, History, ORIGINAL, Snapshot, StoreError, Version};
use crate::develop::white;
use crate::raw::DevelopedRaw;

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
        let code = match &e {
            StoreError::NoSuchStep(_) => "historyStepMissing",
            StoreError::NoSuchVersion(_) => "versionMissing",
            StoreError::NoSuchSnapshot(_) => "snapshotMissing",
            StoreError::NameTaken(_) => "nameTaken",
            StoreError::InvalidName => "invalidName",
            _ => "internal",
        };
        Self::new(code, e)
    }
}

struct Session {
    media_id: i64,
    /// Image de travail avant géométrie (orientation du boîtier seulement).
    base: WorkingImage,
    /// Image de travail transformée par `geometry`, celle qui est rendue.
    image: WorkingImage,
    geometry: Geometry,
    gpu: Option<GpuImage>,
    /// Original en pleine définition, décodé au premier zoom 100 %.
    full: Option<Arc<DevelopedRaw>>,
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
    /// Préréglages de balance des blancs et leurs valeurs.
    pub presets: Vec<WhitePreset>,
    /// Processeur graphique utilisé, ou rien pour le rendu processeur.
    pub adapter: Option<String>,
    pub decode_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhitePreset {
    pub name: &'static str,
    pub temperature: f64,
    pub tint: f64,
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
        let base = source::working_image(&conn, media_id, PREVIEW_SIDE)?;
        let decode_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let settings = store::load(&conn, media_id, ORIGINAL)?;
        let geometry = settings.geometry.clone();
        let image = geometry::transform(&base, &geometry);
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
            presets: white::PRESETS
                .iter()
                .map(|&(name, temperature, tint)| WhitePreset {
                    name,
                    temperature,
                    tint,
                })
                .collect(),
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
        *session = Some(Session {
            media_id,
            base,
            image,
            geometry,
            gpu,
            full: None,
        });
        Ok(opened)
    })
    .await
}

/// Rendu au GPU si possible, au processeur sinon, puis mis en forme pour
/// l'interface : en-tête, histogramme, pixels.
fn render_frame(
    engine: &DevelopEngine,
    image: &WorkingImage,
    uploaded: Option<&GpuImage>,
    params: &RenderParams,
) -> Vec<u8> {
    render_frame_cut(engine, image, uploaded, params, None)
}

/// Comme [`render_frame`], en ne gardant que la partie `cut` (gauche, haut,
/// largeur, hauteur) de l'image rendue : la marge autour d'une région zoomée.
fn render_frame_cut(
    engine: &DevelopEngine,
    image: &WorkingImage,
    uploaded: Option<&GpuImage>,
    params: &RenderParams,
    cut: Option<(u32, u32, u32, u32)>,
) -> Vec<u8> {
    let started = Instant::now();
    let gpu = engine.gpu().zip(uploaded).and_then(|(r, img)| {
        r.render(img, params)
            .inspect_err(|e| eprintln!("rendu au processeur : {e}"))
            .ok()
    });
    let on_gpu = gpu.is_some();
    let rendered = gpu.unwrap_or_else(|| render(image, params));
    let micros = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
    let (cx, cy, cw, ch) = cut.unwrap_or((0, 0, image.width, image.height));
    let mut out =
        Vec::with_capacity(HEADER + rendered.histogram.len() * 4 + (cw * ch * 4) as usize);
    for v in [cw, ch, micros, u32::from(on_gpu)] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in &rendered.histogram {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for y in cy..cy + ch {
        let start = ((y * image.width + cx) * 4) as usize;
        out.extend_from_slice(&rendered.rgba[start..start + (cw * 4) as usize]);
    }
    out
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
        let mut guard = engine
            .session
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        let session = guard
            .as_mut()
            .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?;
        let settings = settings.sanitized();
        // Nouvelle géométrie : l'image de travail est rééchantillonnée, une fois.
        if !settings.geometry.same_image(&session.geometry) {
            session.image = geometry::transform(&session.base, &settings.geometry);
            session.gpu = engine.gpu().and_then(|r| {
                r.upload(&session.image)
                    .inspect_err(|e| eprintln!("rendu au processeur : {e}"))
                    .ok()
            });
            session.geometry = settings.geometry.clone();
        }
        let params = RenderParams::from_settings(
            &settings,
            session.image.as_shot_white,
            clipping.unwrap_or_default(),
        );
        Ok(render_frame(
            &engine,
            &session.image,
            session.gpu.as_ref(),
            &params,
        ))
    })
    .await?;
    Ok(Response::new(bytes))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomOpened {
    /// Dimensions de l'image rendue en pleine définition (après géométrie).
    pub width: u32,
    pub height: u32,
    pub decode_ms: u64,
}

/// Prépare le zoom 100 % : décode l'original en pleine définition (une fois
/// par photo ouverte ; de 1 à 3 s selon le capteur).
#[tauri::command]
pub async fn develop_zoom_open(
    app: AppHandle,
    state: State<'_, AppState>,
    geometry: Option<Geometry>,
) -> CommandResult<ZoomOpened> {
    let geometry = geometry.unwrap_or_default();
    let conn = state.connect()?;
    blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let (media_id, ready) = {
            let session = engine
                .session
                .lock()
                .map_err(|e| CommandError::new("internal", e))?;
            let session = session
                .as_ref()
                .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?;
            (session.media_id, session.full.clone())
        };
        let opened = |full: &DevelopedRaw, decode_ms| {
            let out = geometry::plan(&geometry, f64::from(full.width), f64::from(full.height));
            ZoomOpened {
                width: out.width,
                height: out.height,
                decode_ms,
            }
        };
        if let Some(full) = ready {
            return Ok(opened(&full, 0));
        }
        // Décodage hors du verrou : le rendu de l'aperçu continue pendant ce temps.
        let started = Instant::now();
        let full = Arc::new(source::full_resolution(&conn, media_id)?);
        let decode_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut session = engine
            .session
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        // La photo a pu changer pendant le décodage : le résultat ne sert plus.
        match session.as_mut() {
            Some(s) if s.media_id == media_id => s.full = Some(Arc::clone(&full)),
            _ => {
                return Err(CommandError::new(
                    "internal",
                    "photo changée pendant le décodage",
                ));
            }
        }
        Ok(opened(&full, decode_ms))
    })
    .await
}

/// Rend une région de l'original en pleine définition (zoom 100 %), dans le
/// même format que [`develop_render`].
#[tauri::command]
pub async fn develop_zoom_render(
    app: AppHandle,
    settings: DevelopSettings,
    clipping: Option<Clipping>,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> CommandResult<Response> {
    let bytes = blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let full = {
            let session = engine
                .session
                .lock()
                .map_err(|e| CommandError::new("internal", e))?;
            session
                .as_ref()
                .and_then(|s| s.full.clone())
                .ok_or_else(|| CommandError::new("internal", "zoom non préparé"))?
        };
        let settings = settings.sanitized();
        let params = RenderParams::from_settings(
            &settings,
            full.as_shot_white,
            clipping.unwrap_or_default(),
        );
        // Réduction du bruit et netteté regardent les voisins : la région est
        // rendue avec une marge, puis rognée, pour des bords justes.
        let (region, cut) = WorkingImage::region_with_margin(
            &full,
            &params,
            &settings.geometry,
            left,
            top,
            width,
            height,
        );
        let uploaded = engine.gpu().and_then(|r| r.upload(&region).ok());
        Ok(render_frame_cut(
            &engine,
            &region,
            uploaded.as_ref(),
            &params,
            Some(cut),
        ))
    })
    .await?;
    Ok(Response::new(bytes))
}

/// Réglages automatiques de la section Lumière, depuis les réglages actuels ;
/// `key` : un seul curseur (Maj + double-clic), les autres restent tels quels.
#[tauri::command]
pub async fn develop_auto_light(
    app: AppHandle,
    settings: DevelopSettings,
    key: Option<String>,
) -> CommandResult<DevelopSettings> {
    blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let session = engine
            .session
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        let image = &session
            .as_ref()
            .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?
            .image;
        let mut out = settings.sanitized();
        let light = auto::auto_light(image, &out);
        let l = &mut out.light;
        match key.as_deref() {
            None => *l = light,
            Some("exposure") => l.exposure = light.exposure,
            Some("contrast") => l.contrast = light.contrast,
            Some("highlights") => l.highlights = light.highlights,
            Some("shadows") => l.shadows = light.shadows,
            Some("whites") => l.whites = light.whites,
            Some("blacks") => l.blacks = light.blacks,
            Some(other) => {
                return Err(CommandError::new(
                    "internal",
                    format!("pas d'auto pour {other}"),
                ));
            }
        }
        Ok(out)
    })
    .await
}

/// Balance des blancs automatique de la photo ouverte.
#[tauri::command]
pub async fn develop_auto_white(app: AppHandle) -> CommandResult<WhiteBalance> {
    blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let session = engine
            .session
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        let image = &session
            .as_ref()
            .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?
            .image;
        Ok(auto::auto_white(image))
    })
    .await
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

/// Version visée par une commande : l'originale par défaut.
fn version_name(version: Option<String>) -> String {
    version.unwrap_or_else(|| ORIGINAL.to_owned())
}

/// Opération qui modifie les retouches : exécutée hors du fil de l'interface,
/// puis réveil du fil d'écriture des XMP.
async fn mutate<T, F>(state: &AppState, writer: &XmpWriter, f: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut rusqlite::Connection) -> Result<T, StoreError> + Send + 'static,
{
    let mut conn = state.connect()?;
    let out = blocking(move || Ok(f(&mut conn)?)).await?;
    writer.wake();
    Ok(out)
}

/// Enregistre les réglages (base, historique, XMP au passage suivant du fil d'écriture).
#[tauri::command]
pub async fn develop_save(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    version: Option<String>,
    settings: DevelopSettings,
    label: String,
) -> CommandResult<DevelopSettings> {
    let version = version_name(version);
    mutate(&state, &writer, move |c| {
        store::save(c, media_id, &version, settings, &label)
    })
    .await
}

/// Historique d'une version et position courante.
#[tauri::command]
pub async fn develop_history(
    state: State<'_, AppState>,
    media_id: i64,
    version: Option<String>,
) -> CommandResult<History> {
    let conn = state.connect()?;
    let version = version_name(version);
    blocking(move || Ok(store::history_of(&conn, media_id, &version)?)).await
}

/// Revient à une étape de l'historique (annuler, rétablir, clic dans la
/// liste ; 0 = la photo d'origine) et renvoie ses réglages.
#[tauri::command]
pub async fn develop_go_to(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    version: Option<String>,
    position: i64,
) -> CommandResult<DevelopSettings> {
    let version = version_name(version);
    mutate(&state, &writer, move |c| {
        store::go_to(c, media_id, &version, position)
    })
    .await
}

/// Tout ce que le Retoucher affiche d'une version : réglages, historique,
/// instantanés, et la liste des copies virtuelles de la photo.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionState {
    pub settings: DevelopSettings,
    pub history: History,
    pub snapshots: Vec<Snapshot>,
    pub versions: Vec<Version>,
}

#[tauri::command]
pub async fn develop_state(
    state: State<'_, AppState>,
    media_id: i64,
    version: Option<String>,
) -> CommandResult<VersionState> {
    let conn = state.connect()?;
    let version = version_name(version);
    blocking(move || {
        Ok(VersionState {
            settings: store::load(&conn, media_id, &version)?,
            history: store::history_of(&conn, media_id, &version)?,
            snapshots: store::snapshots(&conn, media_id, &version)?,
            versions: store::versions(&conn, media_id)?,
        })
    })
    .await
}

/// Crée une copie virtuelle à partir de la version `from`.
#[tauri::command]
pub async fn develop_version_create(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    from: Option<String>,
    name: String,
) -> CommandResult<()> {
    let from = version_name(from);
    mutate(&state, &writer, move |c| {
        store::create_version(c, media_id, &from, &name).map(|_| ())
    })
    .await
}

#[tauri::command]
pub async fn develop_version_rename(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    from: String,
    to: String,
) -> CommandResult<()> {
    mutate(&state, &writer, move |c| {
        store::rename_version(c, media_id, &from, &to)
    })
    .await
}

#[tauri::command]
pub async fn develop_version_delete(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    name: String,
) -> CommandResult<()> {
    mutate(&state, &writer, move |c| {
        store::delete_version(c, media_id, &name)
    })
    .await
}

#[tauri::command]
pub async fn develop_snapshot_create(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    version: Option<String>,
    name: String,
) -> CommandResult<()> {
    let version = version_name(version);
    mutate(&state, &writer, move |c| {
        store::create_snapshot(c, media_id, &version, &name)
    })
    .await
}

/// Applique un instantané (une étape d'historique, donc annulable).
#[tauri::command]
pub async fn develop_snapshot_apply(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    version: Option<String>,
    name: String,
) -> CommandResult<DevelopSettings> {
    let version = version_name(version);
    mutate(&state, &writer, move |c| {
        store::apply_snapshot(c, media_id, &version, &name)
    })
    .await
}

#[tauri::command]
pub async fn develop_snapshot_rename(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    version: Option<String>,
    from: String,
    to: String,
) -> CommandResult<()> {
    let version = version_name(version);
    mutate(&state, &writer, move |c| {
        store::rename_snapshot(c, media_id, &version, &from, &to)
    })
    .await
}

#[tauri::command]
pub async fn develop_snapshot_delete(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_id: i64,
    version: Option<String>,
    name: String,
) -> CommandResult<()> {
    let version = version_name(version);
    mutate(&state, &writer, move |c| {
        store::delete_snapshot(c, media_id, &version, &name)
    })
    .await
}

/// Recadrage ramené dans l'image redressée, sans coin vide (voir
/// `geometry::constrain`), aux proportions de la photo ouverte.
#[tauri::command]
pub fn develop_geometry_constrain(
    engine: State<'_, DevelopEngine>,
    geometry: Geometry,
) -> CommandResult<Geometry> {
    let session = engine
        .session
        .lock()
        .map_err(|e| CommandError::new("internal", e))?;
    let base = &session
        .as_ref()
        .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?
        .base;
    let mut g = geometry;
    g.crop = geometry::constrain(&g, f64::from(base.width), f64::from(base.height));
    Ok(g)
}

/// Horizon automatique : angle de redressement, en degrés, pour les quarts de
/// tour et miroirs donnés.
#[tauri::command]
pub async fn develop_auto_straighten(app: AppHandle, geometry: Geometry) -> CommandResult<f64> {
    blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let session = engine
            .session
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        let base = &session
            .as_ref()
            .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?
            .base;
        let oriented = Geometry {
            quarter: geometry.quarter,
            flip_h: geometry.flip_h,
            flip_v: geometry.flip_v,
            ..Geometry::default()
        };
        Ok(geometry::auto_straighten(&geometry::transform(
            base, &oriented,
        )))
    })
    .await
}
