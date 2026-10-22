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
use super::previews::PreviewQueue;
use super::{AppState, blocking};
use crate::develop::auto;
use crate::develop::geometry;
use crate::develop::gpu::{GpuImage, GpuRenderer};
use crate::develop::lens::{self, LensCorrection, LensProfile};
use crate::develop::pipeline::{Clipping, RenderParams, WorkingImage, render};
use crate::develop::presets;
use crate::develop::settings::{DevelopSettings, Geometry, LensSettings, Spot, WhiteBalance};
use crate::develop::source::{self, PREVIEW_SIDE, SourceError};
use crate::develop::spots;
use crate::develop::store::{self, History, ORIGINAL, Snapshot, StoreError, Version};
use crate::develop::white;
use crate::library::copies;
use crate::library::media::is_stale;
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
    /// Image de travail décodée (orientation du boîtier seulement).
    base: WorkingImage,
    /// Objectif reconnu dans la base lensfun.
    profile: Option<LensProfile>,
    lens: LensSettings,
    /// `base` corrigée de l'objectif selon `lens` : repère des corrections de
    /// défauts et de la géométrie.
    corrected: WorkingImage,
    /// Image de travail corrigée par `spots` puis transformée par
    /// `geometry`, celle qui est rendue.
    image: WorkingImage,
    geometry: Geometry,
    spots: Vec<Spot>,
    gpu: Option<GpuImage>,
    /// `image` réduite pour les vignettes des presets, faite au premier besoin.
    thumb: Option<WorkingImage>,
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
    /// Image avant géométrie, repère des corrections de défauts.
    pub source_width: u32,
    pub source_height: u32,
    /// Objectif reconnu (corrections d'objectif possibles), ou rien.
    pub lens: Option<String>,
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
        let profile = lens::find_profile(&source::shot(&conn, media_id)?);
        let corrected = lens_corrected(&base, profile.as_ref(), &settings.lens);
        let lens_settings = settings.lens.clone();
        let geometry = settings.geometry.clone();
        let spot_list = settings.spots.clone();
        let image = geometry::transform(&spots::healed(&corrected, &spot_list), &geometry);
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
            source_width: base.width,
            source_height: base.height,
            lens: profile.as_ref().map(|p| p.name.clone()),
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
        let previous = session.as_ref().map(|s| s.media_id);
        if let Some(previous) = previous.filter(|p| *p != media_id) {
            refresh_previews(&app, previous);
        }
        *session = Some(Session {
            media_id,
            base,
            profile,
            lens: lens_settings,
            corrected,
            image,
            geometry,
            spots: spot_list,
            gpu,
            thumb: None,
            full: None,
        });
        Ok(opened)
    })
    .await
}

/// Corrections d'objectif d'une image décodée de w × h pixels, ou rien.
fn lens_correction(
    profile: Option<&LensProfile>,
    settings: &LensSettings,
    w: u32,
    h: u32,
) -> Option<LensCorrection> {
    profile
        .and_then(|p| LensCorrection::new(p, settings))
        .map(|c| c.for_image(w, h))
}

/// L'image décodée corrigée de l'objectif (aperçu).
fn lens_corrected(
    base: &WorkingImage,
    profile: Option<&LensProfile>,
    settings: &LensSettings,
) -> WorkingImage {
    match lens_correction(profile, settings, base.width, base.height) {
        Some(c) => c.whole(base),
        None => base.clone(),
    }
}

/// Rendu au GPU si possible, au processeur sinon, puis mis en forme pour
/// l'interface : en-tête, histogramme, pixels. `cut` (gauche, haut, largeur,
/// hauteur) ne garde qu'une partie de l'image rendue : sans la marge autour
/// d'une région zoomée. `dust` : visualisation des poussières, peinte après
/// l'histogramme.
fn render_frame_cut(
    engine: &DevelopEngine,
    image: &WorkingImage,
    uploaded: Option<&GpuImage>,
    params: &RenderParams,
    cut: Option<(u32, u32, u32, u32)>,
    dust: bool,
) -> Vec<u8> {
    let started = Instant::now();
    let gpu = engine.gpu().zip(uploaded).and_then(|(r, img)| {
        r.render(img, params)
            .inspect_err(|e| eprintln!("rendu au processeur : {e}"))
            .ok()
    });
    let on_gpu = gpu.is_some();
    let mut rendered = gpu.unwrap_or_else(|| render(image, params));
    if dust {
        let (_, _, fw, fh) = image.frame();
        spots::paint_dust(&mut rendered.rgba, image.width, image.height, fw.max(fh));
    }
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
        // Nouvelles corrections d'objectif : l'image corrigée est refaite.
        let lens_changed = settings.lens != session.lens;
        if lens_changed {
            session.corrected =
                lens_corrected(&session.base, session.profile.as_ref(), &settings.lens);
            session.lens = settings.lens.clone();
        }
        // Nouvelle géométrie ou nouvelles corrections : l'image de travail est
        // refaite, une fois.
        if lens_changed
            || !settings.geometry.same_image(&session.geometry)
            || settings.spots != session.spots
        {
            session.image = geometry::transform(
                &spots::healed(&session.corrected, &settings.spots),
                &settings.geometry,
            );
            session.gpu = engine.gpu().and_then(|r| {
                r.upload(&session.image)
                    .inspect_err(|e| eprintln!("rendu au processeur : {e}"))
                    .ok()
            });
            session.geometry = settings.geometry.clone();
            session.spots = settings.spots.clone();
            session.thumb = None;
        }
        let clipping = clipping.unwrap_or_default();
        let params = RenderParams::from_settings(&settings, session.image.as_shot_white, clipping)
            .rendered(session.image.rendered);
        Ok(render_frame_cut(
            &engine,
            &session.image,
            session.gpu.as_ref(),
            &params,
            None,
            clipping.dust,
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
        let (full, profile) = {
            let session = engine
                .session
                .lock()
                .map_err(|e| CommandError::new("internal", e))?;
            let session = session
                .as_ref()
                .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?;
            let full = session
                .full
                .clone()
                .ok_or_else(|| CommandError::new("internal", "zoom non préparé"))?;
            (full, session.profile.clone())
        };
        let settings = settings.sanitized();
        let clipping = clipping.unwrap_or_default();
        let params = RenderParams::from_settings(&settings, full.as_shot_white, clipping)
            .rendered(full.rendered);
        // Réduction du bruit et netteté regardent les voisins : la région est
        // rendue avec une marge, puis rognée, pour des bords justes.
        let correction = lens_correction(profile.as_ref(), &settings.lens, full.width, full.height);
        let (region, cut) = WorkingImage::region_with_margin(
            &full,
            &params,
            &settings,
            correction.as_ref(),
            (left, top, width, height),
        );
        let uploaded = engine.gpu().and_then(|r| r.upload(&region).ok());
        Ok(render_frame_cut(
            &engine,
            &region,
            uploaded.as_ref(),
            &params,
            Some(cut),
            clipping.dust,
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
pub fn develop_close(app: AppHandle, engine: State<'_, DevelopEngine>) {
    if let Ok(mut session) = engine.session.lock()
        && let Some(previous) = session.take()
    {
        refresh_previews(&app, previous.media_id);
    }
}

/// Photo quittée dans le Retoucher : ses aperçus sont refaits d'après la
/// retouche s'ils sont plus anciens qu'elle (grilles, pellicule, visionneuse).
fn refresh_previews(app: &AppHandle, media_id: i64) {
    let state = app.state::<AppState>();
    let Ok(conn) = state.connect() else {
        return;
    };
    let queue = app.state::<PreviewQueue>();
    if is_stale(&conn, media_id) {
        queue.push(media_id);
    }
    // Ses copies virtuelles retouchées, chacune son aperçu.
    for item in copies::stale_copies(&conn, &state.cache_root, Some(media_id)).unwrap_or_default() {
        queue.push(item);
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
            .corrected;
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

/// Source proposée pour une nouvelle correction de défaut en (x, y), de rayon
/// `radius` (fractions de l'image avant géométrie), loin des autres
/// corrections.
#[tauri::command]
pub async fn develop_spot_source(
    app: AppHandle,
    x: f64,
    y: f64,
    radius: f64,
    others: Vec<Spot>,
) -> CommandResult<[f64; 2]> {
    blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let session = engine
            .session
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        let corrected = &session
            .as_ref()
            .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?
            .corrected;
        let (sx, sy) = spots::find_source(corrected, x, y, radius, &others);
        Ok([sx, sy])
    })
    .await
}

/// Côté long des vignettes de la galerie de filtres, au plus.
const PRESET_THUMB_MAX: u32 = 480;

/// Réglages affichés avec le preset `id` appliqué à `amount` (0 à 100 %) ;
/// `id` absent : « Original », l'aspect appliqué est retiré.
#[tauri::command]
pub async fn develop_preset_apply(
    state: State<'_, AppState>,
    settings: DevelopSettings,
    id: Option<String>,
    amount: f64,
) -> CommandResult<DevelopSettings> {
    let path = super::presets::user_file(&state);
    blocking(move || {
        let settings = settings.sanitized();
        Ok(match id {
            Some(id) => presets::apply(&settings, &presets::find(&path, &id)?, amount),
            None => {
                let current = settings
                    .look
                    .as_ref()
                    .and_then(|l| presets::find(&path, &l.id).ok());
                presets::remove(&settings, current.as_ref())
            }
        })
    })
    .await
}

/// Vignettes de la photo ouverte avec chacun des presets `ids` (chaîne
/// vide : « Original ») à `amount` : nombre (u32 LE), puis pour chacune
/// largeur, hauteur et pixels RGBA.
#[tauri::command]
pub async fn develop_preset_previews(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: DevelopSettings,
    ids: Vec<String>,
    amount: f64,
    size: u32,
) -> CommandResult<Response> {
    let path = super::presets::user_file(&state);
    let bytes = blocking(move || {
        let engine = app.state::<DevelopEngine>();
        let small = {
            let mut guard = engine
                .session
                .lock()
                .map_err(|e| CommandError::new("internal", e))?;
            let session = guard
                .as_mut()
                .ok_or_else(|| CommandError::new("internal", "aucune photo ouverte"))?;
            let side = size.clamp(32, PRESET_THUMB_MAX);
            if session.thumb.as_ref().is_none_or(|t| {
                t.width.max(t.height) != side.min(session.image.width.max(session.image.height))
            }) {
                session.thumb = Some(session.image.downscaled(side));
            }
            session.thumb.clone()
        }
        .ok_or_else(|| CommandError::new("internal", "vignette absente"))?;
        let settings = settings.sanitized();
        let current = settings
            .look
            .as_ref()
            .and_then(|l| presets::find(&path, &l.id).ok());
        let mut out = Vec::new();
        out.extend_from_slice(&(ids.len() as u32).to_le_bytes());
        for id in &ids {
            let applied = if id.is_empty() {
                presets::remove(&settings, current.as_ref())
            } else {
                presets::apply(&settings, &presets::find(&path, id)?, amount)
            };
            let params =
                RenderParams::from_settings(&applied, small.as_shot_white, Clipping::default())
                    .rendered(small.rendered);
            let rendered = render(&small, &params);
            out.extend_from_slice(&small.width.to_le_bytes());
            out.extend_from_slice(&small.height.to_le_bytes());
            out.extend_from_slice(&rendered.rgba);
        }
        Ok(out)
    })
    .await?;
    Ok(Response::new(bytes))
}

/// Copier-coller sur la photo ouverte : ses réglages avec ceux de `from`
/// pour les réglages `controls` (enregistrés ensuite par l'interface).
#[tauri::command]
pub async fn develop_paste_apply(
    state: State<'_, AppState>,
    settings: DevelopSettings,
    from: DevelopSettings,
    controls: Vec<String>,
) -> CommandResult<DevelopSettings> {
    let path = super::presets::user_file(&state);
    blocking(move || {
        Ok(presets::paste(
            &path,
            &settings.sanitized(),
            &from.sanitized(),
            &controls,
        )?)
    })
    .await
}

/// Copier-coller sur d'autres photos (leur version originale) : chacune
/// reçoit une étape d'historique « paste » et son XMP est mis à jour.
/// Renvoie le nombre de photos modifiées.
#[tauri::command]
pub async fn develop_paste(
    app: AppHandle,
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    media_ids: Vec<i64>,
    from: DevelopSettings,
    controls: Vec<String>,
) -> CommandResult<u32> {
    let path = super::presets::user_file(&state);
    let from = from.sanitized();
    let mut conn = state.connect()?;
    let changed = blocking(move || {
        let mut changed = Vec::new();
        for id in media_ids {
            // Une copie virtuelle reçoit le collage dans sa version.
            let Some((media, version)) = copies::resolve(&conn, id).map_err(StoreError::from)?
            else {
                continue;
            };
            let current = store::load(&conn, media, &version)?;
            let next = presets::paste(&path, &current, &from, &controls)?;
            if next != current {
                store::save(&mut conn, media, &version, next, "paste")?;
                changed.push(id);
            }
        }
        Ok(changed)
    })
    .await?;
    writer.wake();
    // Aperçus refaits d'après la nouvelle retouche.
    let queue = app.state::<PreviewQueue>();
    for id in &changed {
        queue.push(*id);
    }
    Ok(changed.len() as u32)
}
