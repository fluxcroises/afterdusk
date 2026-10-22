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
use super::render::{RenderOptions, Resize, Sharpen, render_photo};
use super::templates::{self, Template};
use crate::db;
use crate::develop::lens::{self, Shot};
use crate::develop::store;
use crate::import::copy::copy_verified;
use crate::library::copies;
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
    /// Modèle de livraison : taille, cadrage et accentuation du modèle, le
    /// reste (format, couleurs, métadonnées, filigrane) du preset.
    #[serde(default)]
    pub template: Option<String>,
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
pub(crate) struct Photo {
    /// Identifiant de grille : négatif pour une copie virtuelle.
    item: i64,
    media_id: i64,
    /// Version exportée : vide pour l'originale, nom d'une copie virtuelle.
    version: String,
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

fn photo(conn: &Connection, item: i64) -> Result<Photo, PhotoError> {
    let (id, version) = copies::resolve(conn, item)
        .map_err(|e| (item.to_string(), "internal".to_owned(), e.to_string()))?
        .ok_or_else(|| (item.to_string(), "mediaNotFound".to_owned(), String::new()))?;
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
        item,
        media_id: id,
        version,
        path,
        sidecar,
        name,
        original,
        captured_at,
    })
}

/// Exporte une photo avec un preset ; renvoie le fichier écrit, ou rien
/// (copie virtuelle avec le preset des originaux : le fichier est le même).
fn export_one(
    conn: &Connection,
    photo: &Photo,
    preset: &ExportPreset,
    template: Option<&Template>,
    settings: &ExportSettings,
    dir: &Path,
    seq: usize,
) -> Result<Option<PathBuf>, ExportError> {
    let mut stem = presets::file_stem(
        &preset.naming,
        &photo.name,
        &photo.original,
        photo.captured_at.as_deref(),
        seq,
    );
    if !photo.version.is_empty() {
        stem = format!("{stem} - {}", presets::clean_name(&photo.version));
    }
    std::fs::create_dir_all(dir)?;
    let Some(format) = preset.output.format() else {
        if !photo.version.is_empty() {
            return Ok(None);
        }
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
        return Ok(Some(target));
    };
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
    let image = match template {
        Some(t) => render_template(conn, photo, t, color, mark)?,
        None => render_source(conn, photo, options, mark)?,
    };
    write_rendered(conn, photo, &image, preset, format, color, dir, &stem).map(Some)
}

/// Taille retouchée d'une photo de la grille.
pub fn item_size(conn: &Connection, item: i64) -> Result<(u32, u32), ExportError> {
    let p = photo(conn, item)
        .map_err(|(_, code, detail)| ExportError::Settings(format!("{code} {detail}")))?;
    full_size(conn, &p)
}

/// Taille de la photo retouchée (cadrage de Développer et orientation compris).
fn full_size(conn: &Connection, photo: &Photo) -> Result<(u32, u32), ExportError> {
    match format_of(&photo.path) {
        Some(MediaFormat::Raw) => {
            let develop = store::load(conn, photo.media_id, &photo.version)
                .map_err(|e| ExportError::Settings(e.to_string()))?;
            let raw = RawFile::open(&photo.path)?;
            Ok(super::render::output_size(&raw, &develop, Resize::Full))
        }
        Some(MediaFormat::Image) => {
            let (w, h) = image::image_dimensions(&photo.path)
                .map_err(|e| ExportError::Unsupported(format!("{} : {e}", photo.name)))?;
            let orientation: u8 = conn
                .query_row(
                    "SELECT orientation FROM media WHERE id = ?1",
                    [photo.media_id],
                    |r| r.get(0),
                )
                .unwrap_or(1);
            Ok(if orientation >= 5 { (h, w) } else { (w, h) })
        }
        _ => Err(ExportError::Unsupported(photo.name.clone())),
    }
}

/// Rendu pour un modèle : la photo juste assez grande pour le cadre, le
/// cadre découpé (enregistré, sinon sur le plus grand visage), mis à la
/// taille du modèle, puis accentué.
fn render_template(
    conn: &Connection,
    photo: &Photo,
    template: &Template,
    color: ColorSpace,
    mark: Option<&super::watermark::Watermark>,
) -> Result<super::render::Rendered, ExportError> {
    let (fw, fh) = full_size(conn, photo)?;
    let (tw, th) = template.size_for(fw, fh);
    let ratio = f64::from(tw) / f64::from(th);
    let saved = templates::get(conn, photo.item, template.id)
        .map_err(|e| ExportError::Settings(e.to_string()))?
        .map(|r| templates::conform(r, ratio, fw, fh));
    let span = saved.unwrap_or_else(|| templates::fit(ratio, fw, fh, (0.5, 0.5)));
    let width = (f64::from(tw) / (span[2] - span[0]).max(0.01)).ceil() as u32;
    let options = RenderOptions {
        resize: Resize::Width(width),
        sharpen: Sharpen::None,
        color: ColorSpace::Srgb,
    };
    let image = render_source(conn, photo, options, None)?;
    let rect = saved.unwrap_or_else(|| {
        let small = image::RgbImage::from_raw(image.width, image.height, image.to_u8())
            .map(|img| crate::raw::preview::fit(&img, 1024));
        let face = small.as_ref().and_then(templates::subject);
        templates::default_frame(ratio, image.width, image.height, face)
    });
    let framed = super::render::crop_fraction(&image, rect);
    drop(image);
    let sized = super::render::resize_exact(framed, tw, th);
    super::render::finish(sized, Resize::Full, template.sharpen(), color, mark)
}

/// Rend une photo de la bibliothèque : un RAW avec ses réglages, une autre
/// image (JPEG, TIFF, PNG) telle quelle, orientation appliquée.
pub(crate) fn render_source(
    conn: &Connection,
    photo: &Photo,
    options: RenderOptions,
    mark: Option<&super::watermark::Watermark>,
) -> Result<super::render::Rendered, ExportError> {
    match format_of(&photo.path) {
        Some(MediaFormat::Raw) => {
            let develop = store::load(conn, photo.media_id, &photo.version)
                .map_err(|e| ExportError::Settings(e.to_string()))?;
            let mut raw = RawFile::open(&photo.path)?;
            let profile = lens::find_profile(&Shot::from(&raw.info()));
            render_photo(&mut raw, &develop, profile.as_ref(), options, mark)
        }
        Some(MediaFormat::Image) => {
            let orientation: u8 = conn
                .query_row(
                    "SELECT orientation FROM media WHERE id = ?1",
                    [photo.media_id],
                    |r| r.get(0),
                )
                .unwrap_or(1);
            let img = super::render::decode_image(&photo.path, orientation)?;
            super::render::finish(img, options.resize, options.sharpen, options.color, mark)
        }
        _ => Err(ExportError::Unsupported(photo.name.clone())),
    }
}

/// Métadonnées et écriture du fichier rendu.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_rendered(
    conn: &Connection,
    photo: &Photo,
    image: &super::render::Rendered,
    preset: &ExportPreset,
    format: encode::Format,
    color: ColorSpace,
    dir: &Path,
    stem: &str,
) -> Result<PathBuf, ExportError> {
    let source_format = format_of(&photo.path).unwrap_or(MediaFormat::Image);
    let mut camera = read_metadata(&photo.path, source_format);
    // Lieu posé après coup (trace, téléphone, saisie) : il suit la photo,
    // sauf si le preset retire le GPS.
    if camera.gps.is_none() {
        camera.gps = conn
            .query_row(
                "SELECT lat, lon FROM media WHERE id = ?1 AND lat IS NOT NULL",
                [photo.media_id],
                |r| {
                    Ok(crate::raw::Gps {
                        latitude: r.get(0)?,
                        longitude: r.get(1)?,
                        altitude: None,
                    })
                },
            )
            .ok();
    }
    // Heure corrigée d'après le lieu : c'est elle qui part dans le fichier.
    if let Ok((at, offset)) = conn.query_row(
        "SELECT captured_at, utc_offset FROM media
         WHERE id = ?1 AND captured_at_camera IS NOT NULL",
        [photo.media_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
    ) {
        camera.captured_at = Some(at);
        camera.utc_offset = offset;
    }
    let meta = PhotoMeta {
        camera,
        sidecar: photo
            .sidecar
            .as_ref()
            .filter(|p| p.exists())
            .and_then(|p| read_sidecar(p).ok().flatten()),
    };
    let embedded = metadata::embedded(&meta, preset.metadata)?;
    let target = presets::free_path(dir, stem, format.extension());
    encode::write(image, format, &color.icc()?, &embedded, &target)?;
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
    let targets: Vec<(&ExportPreset, Option<&Template>, PathBuf)> = request
        .targets
        .iter()
        .filter_map(|t| {
            let preset = settings.presets.iter().find(|p| p.id == t.preset)?;
            let template = match &t.template {
                Some(id) => Some(templates::find(id)?),
                None => None,
            };
            let dir = match (&t.folder, request.targets.len() > 1) {
                (Some(f), true) => request
                    .destination
                    .join(presets::file_stem(f, f, f, None, 0)),
                _ => request.destination.clone(),
            };
            Some((preset, template, dir))
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
    let presets_only: Vec<&ExportPreset> = targets.iter().map(|(p, _, _)| *p).collect();
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
                            for (preset, template, dir) in &targets {
                                if cancel.load(Ordering::SeqCst) {
                                    break;
                                }
                                if let Err(e) = export_one(
                                    &conn,
                                    &p,
                                    preset,
                                    *template,
                                    settings,
                                    dir,
                                    index + 1,
                                ) {
                                    failures.push(Failure {
                                        file: p.name.clone(),
                                        preset: template
                                            .map_or_else(|| preset.id.clone(), |t| t.id.to_owned()),
                                        code: code_of(&e).into(),
                                        detail: e.to_string(),
                                    });
                                }
                            }
                        }
                        Err((file, code, detail)) => {
                            for (preset, _, _) in &targets {
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

/// Taille du fichier exporté, et JPEG réduit (absent pour les originaux).
pub type Preview = ((u32, u32), Option<Vec<u8>>);

/// Aperçu du résultat d'un preset sur une photo (écran Livrer) : taille du
/// fichier exporté, et l'image réduite à `max_side`, filigrane compris.
/// `None` pour l'image : le preset des originaux (le fichier tel quel).
pub fn preview(
    conn: &Connection,
    item: i64,
    preset: &ExportPreset,
    watermark: Option<&super::watermark::Watermark>,
    max_side: u32,
) -> Result<Preview, ExportError> {
    let p = photo(conn, item)
        .map_err(|(_, code, detail)| ExportError::Settings(format!("{code} {detail}")))?;
    if preset.output.format().is_none() {
        return Ok(((0, 0), None));
    }
    let size = match format_of(&p.path) {
        Some(MediaFormat::Raw) => {
            let develop = store::load(conn, p.media_id, &p.version)
                .map_err(|e| ExportError::Settings(e.to_string()))?;
            let raw = RawFile::open(&p.path)?;
            super::render::output_size(&raw, &develop, preset.resize)
        }
        Some(MediaFormat::Image) => {
            let (w, h) = image::image_dimensions(&p.path)
                .map_err(|e| ExportError::Unsupported(format!("{} : {e}", p.name)))?;
            let orientation: u8 = conn
                .query_row(
                    "SELECT orientation FROM media WHERE id = ?1",
                    [p.media_id],
                    |r| r.get(0),
                )
                .unwrap_or(1);
            let (w, h) = if orientation >= 5 { (h, w) } else { (w, h) };
            super::render::target_size(w, h, preset.resize)
        }
        _ => return Ok(((0, 0), None)),
    };
    let options = RenderOptions {
        resize: Resize::LongSide(size.0.max(size.1).min(max_side)),
        sharpen: preset.sharpen,
        color: ColorSpace::Srgb,
    };
    let mark = watermark.filter(|_| preset.watermark);
    let img = render_source(conn, &p, options, mark)?;
    let mut jpeg = Vec::new();
    let enc = |e: jpeg_encoder::EncodingError| ExportError::Encode(e.to_string());
    let (w, h) = (
        u16::try_from(img.width).map_err(|_| ExportError::TooLarge)?,
        u16::try_from(img.height).map_err(|_| ExportError::TooLarge)?,
    );
    jpeg_encoder::Encoder::new(&mut jpeg, 88)
        .encode(&img.to_u8(), w, h, jpeg_encoder::ColorType::Rgb)
        .map_err(enc)?;
    Ok((size, Some(jpeg)))
}

/// Photo de la grille rendue pour remplir exactement `w` × `h` : juste assez
/// grande, découpée autour de `focus` (fractions), accentuée.
pub(crate) fn render_cover(
    conn: &Connection,
    item: i64,
    w: u32,
    h: u32,
    focus: [f64; 2],
    sharpen: Sharpen,
) -> Result<super::render::Rendered, ExportError> {
    let p = photo(conn, item)
        .map_err(|(file, code, detail)| ExportError::Settings(format!("{file} {code} {detail}")))?;
    let (fw, fh) = full_size(conn, &p)?;
    let scale = (f64::from(w) / f64::from(fw.max(1))).max(f64::from(h) / f64::from(fh.max(1)));
    let width = (f64::from(fw) * scale).ceil() as u32;
    let options = RenderOptions {
        resize: Resize::Width(width.max(1)),
        sharpen: Sharpen::None,
        color: ColorSpace::Srgb,
    };
    let image = render_source(conn, &p, options, None)?;
    let ratio = f64::from(w) / f64::from(h.max(1));
    let rect = templates::fit(ratio, image.width, image.height, (focus[0], focus[1]));
    let framed = super::render::crop_fraction(&image, rect);
    drop(image);
    let sized = super::render::resize_exact(framed, w, h);
    super::render::finish(sized, Resize::Full, sharpen, ColorSpace::Srgb, None)
}

/// Preset qui donne le format, les couleurs et les métadonnées d'un usage.
pub fn group_preset(group: templates::Group) -> &'static str {
    match group {
        templates::Group::Social => "instagram",
        templates::Group::Print => "full",
        templates::Group::Web => "web",
    }
}

/// Exporte une composition à pleine résolution dans `dir`, sous `name`.
/// Format et couleurs du preset de son usage ; métadonnées de sa première
/// photo (auteur, copyright), filtrées par ce preset.
pub fn export_design(
    conn: &Connection,
    settings: &ExportSettings,
    design: &super::design::Design,
    dir: &Path,
    name: &str,
) -> Result<PathBuf, ExportError> {
    let template = templates::find(&design.template)
        .ok_or_else(|| ExportError::Settings(format!("modèle inconnu : {}", design.template)))?;
    let preset_id = group_preset(template.group);
    let preset = settings
        .presets
        .iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| ExportError::Settings(format!("preset absent : {preset_id}")))?;
    // Un preset réglé sur les originaux n'a pas de format rendu : JPEG.
    let format = preset
        .output
        .format()
        .unwrap_or(encode::Format::Jpeg { quality: 95 });
    let color = if matches!(preset.output, Output::Avif { .. }) {
        ColorSpace::Srgb
    } else {
        preset.color
    };
    let first = design
        .photos
        .first()
        .ok_or_else(|| ExportError::Settings("composition sans photo".into()))?;
    let lead = photo(conn, first.item)
        .map_err(|(file, code, detail)| ExportError::Settings(format!("{file} {code} {detail}")))?;
    let mut page = super::design::compose(design, |p, w, h| {
        render_cover(conn, p.item, w, h, p.focus, template.sharpen())
    })?;
    color.convert(&mut page.rgb)?;
    std::fs::create_dir_all(dir)?;
    let stem = presets::clean_name(name);
    let stem = if stem.is_empty() {
        "Composition".to_owned()
    } else {
        stem
    };
    write_rendered(conn, &lead, &page, preset, format, color, dir, &stem)
}
