//! Yeux fermés (décision 0010) : YuNet trouve les visages et leurs yeux,
//! Face Mesh V2 place 478 repères sur chaque visage recadré droit, et le
//! rapport d'ouverture de chaque œil (EAR, hauteur des paupières sur largeur)
//! dit s'il est fermé. ONNX Runtime est livré avec l'application et chargé
//! une fois ; sans lui, les yeux ne sont simplement pas analysés.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use image::RgbImage;
use image::imageops::{self, FilterType};
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;

const YUNET_MODEL: &[u8] = include_bytes!("../../assets/models/face_detection_yunet_2023mar.onnx");
const MESH_MODEL: &[u8] = include_bytes!("../../assets/models/face_landmarks.onnx");

/// Nom de la bibliothèque ONNX Runtime livrée (`resources/onnxruntime/`).
#[cfg(target_os = "windows")]
pub const LIBRARY: &str = "onnxruntime.dll";
#[cfg(target_os = "macos")]
pub const LIBRARY: &str = "libonnxruntime.dylib";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub const LIBRARY: &str = "libonnxruntime.so";

/// Côté de l'image donnée à YuNet.
const DETECT_SIDE: usize = 640;
/// Côté du visage recadré donné à Face Mesh.
const MESH_SIDE: usize = 256;
/// Confiance minimale d'un visage.
const MIN_SCORE: f32 = 0.6;
/// Un visage plus petit (en pixels de l'image analysée) ne se juge pas.
const MIN_FACE: f32 = 48.0;
/// Visages examinés au plus, les plus grands d'abord.
const MAX_FACES: usize = 5;
/// Œil fermé en dessous ; il faut les deux yeux pour signaler le visage.
/// Calibré sur des portraits libres : ouverts de 0,24 à 0,38, rire plissé
/// 0,13 et 0,17, fermés de 0,03 à 0,10 (décision 0010).
pub const CLOSED_EAR: f32 = 0.12;

/// Six repères Face Mesh par œil : coins extérieur et intérieur, paupières.
const RIGHT_EYE: [usize; 6] = [33, 160, 158, 133, 153, 144];
const LEFT_EYE: [usize; 6] = [362, 385, 387, 263, 373, 380];

static LIBRARY_PATH: OnceLock<PathBuf> = OnceLock::new();
static ENVIRONMENT: OnceLock<Option<Environment>> = OnceLock::new();
static DETECTOR: OnceLock<Option<Mutex<EyeDetector>>> = OnceLock::new();

/// Emplacement de la bibliothèque, donné au démarrage par l'application.
pub fn set_library_path(path: PathBuf) {
    let _ = LIBRARY_PATH.set(path);
}

fn library_path() -> Option<PathBuf> {
    LIBRARY_PATH
        .get()
        .cloned()
        .or_else(|| std::env::var_os("ORT_DYLIB_PATH").map(PathBuf::from))
        .or_else(|| {
            // Tests et développement : la copie récupérée par le script.
            let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("onnxruntime")
                .join(LIBRARY);
            dev.is_file().then_some(dev)
        })
}

fn environment() -> Option<&'static Environment> {
    ENVIRONMENT
        .get_or_init(|| {
            let path = library_path()?;
            ort::init_from(&path)
                .ok()?
                .with_name("afterdusk")
                .build()
                .ok()
        })
        .as_ref()
}

pub struct EyeDetector {
    detect: Session,
    mesh: Session,
}

fn session(env: &Environment, model: &[u8]) -> Option<Session> {
    // Un seul fil : l'analyse tourne en arrière-plan sans ralentir le tri.
    let mut builder = Session::builder(env).ok()?.with_intra_threads(1).ok()?;
    builder.commit_from_memory(model).ok()
}

/// Le détecteur, chargé à la première demande ; `None` sans ONNX Runtime.
pub fn detector() -> Option<&'static Mutex<EyeDetector>> {
    DETECTOR
        .get_or_init(|| {
            let env = environment()?;
            Some(Mutex::new(EyeDetector {
                detect: session(env, YUNET_MODEL)?,
                mesh: session(env, MESH_MODEL)?,
            }))
        })
        .as_ref()
}

#[derive(Debug, Clone, PartialEq)]
struct Face {
    score: f32,
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    /// Œil droit (à gauche dans l'image), œil gauche.
    eyes: [(f32, f32); 2],
}

/// Sorties d'un modèle, dans l'ordre du modèle.
fn run(session: &mut Session, input: Vec<f32>, shape: [usize; 4]) -> Option<Vec<Vec<f32>>> {
    let name = session.inputs().first()?.name().to_owned();
    let tensor = Tensor::from_array((shape.map(|d| d as i64).to_vec(), input)).ok()?;
    let outputs = session.run(ort::inputs![name => tensor]).ok()?;
    outputs
        .iter()
        .map(|(_, v)| v.try_extract_tensor::<f32>().ok().map(|(_, d)| d.to_vec()))
        .collect()
}

fn detect(session: &mut Session, img: &RgbImage) -> Option<Vec<Face>> {
    let scale = DETECT_SIDE as f32 / img.width().max(img.height()) as f32;
    let small = imageops::resize(
        img,
        ((img.width() as f32 * scale) as u32).max(1),
        ((img.height() as f32 * scale) as u32).max(1),
        FilterType::Triangle,
    );
    // BGR de 0 à 255, image en haut à gauche d'un carré noir.
    let plane = DETECT_SIDE * DETECT_SIDE;
    let mut input = vec![0f32; 3 * plane];
    for (x, y, p) in small.enumerate_pixels() {
        let at = y as usize * DETECT_SIDE + x as usize;
        for c in 0..3 {
            input[c * plane + at] = f32::from(p[2 - c]);
        }
    }
    // Sorties : cls, obj, bbox, kps, chacune pour les pas 8, 16 et 32.
    let out = run(session, input, [1, 3, DETECT_SIDE, DETECT_SIDE])?;
    if out.len() < 12 {
        return None;
    }
    let mut faces = Vec::new();
    for (k, stride) in [8usize, 16, 32].into_iter().enumerate() {
        let (cls, obj, bbox, kps) = (&out[k], &out[3 + k], &out[6 + k], &out[9 + k]);
        let cols = DETECT_SIDE / stride;
        let s = stride as f32;
        for i in 0..cls.len().min(obj.len()) {
            let score = (cls[i].clamp(0.0, 1.0) * obj[i].clamp(0.0, 1.0)).sqrt();
            if score < MIN_SCORE || bbox.len() < i * 4 + 4 || kps.len() < i * 10 + 4 {
                continue;
            }
            let (r, c) = ((i / cols) as f32, (i % cols) as f32);
            let point = |j: usize| {
                (
                    (c + kps[i * 10 + 2 * j]) * s / scale,
                    (r + kps[i * 10 + 2 * j + 1]) * s / scale,
                )
            };
            faces.push(Face {
                score,
                cx: (c + bbox[i * 4]) * s / scale,
                cy: (r + bbox[i * 4 + 1]) * s / scale,
                w: bbox[i * 4 + 2].exp() * s / scale,
                h: bbox[i * 4 + 3].exp() * s / scale,
                eyes: [point(0), point(1)],
            });
        }
    }
    // Un visage par zone : le plus sûr ; puis les plus grands d'abord.
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Face> = Vec::new();
    for f in faces {
        if kept
            .iter()
            .all(|k| (k.cx - f.cx).abs() > k.w / 2.0 || (k.cy - f.cy).abs() > k.h / 2.0)
        {
            kept.push(f);
        }
    }
    kept.retain(|f| f.w.max(f.h) >= MIN_FACE);
    kept.sort_by(|a, b| (b.w * b.h).total_cmp(&(a.w * a.h)));
    kept.truncate(MAX_FACES);
    Some(kept)
}

/// Visage recadré droit (yeux à l'horizontale), carré de 1,5 fois la boîte,
/// RVB de 0 à 1.
fn crop(img: &RgbImage, f: &Face) -> Vec<f32> {
    let (r, l) = (f.eyes[0], f.eyes[1]);
    let angle = (l.1 - r.1).atan2(l.0 - r.0);
    let size = f.w.max(f.h) * 1.5;
    let (ca, sa) = (angle.cos(), angle.sin());
    let (wmax, hmax) = (img.width() as f32 - 1.0, img.height() as f32 - 1.0);
    let mut out = vec![0f32; MESH_SIDE * MESH_SIDE * 3];
    for v in 0..MESH_SIDE {
        for u in 0..MESH_SIDE {
            let dx = ((u as f32 + 0.5) / MESH_SIDE as f32 - 0.5) * size;
            let dy = ((v as f32 + 0.5) / MESH_SIDE as f32 - 0.5) * size;
            let x = (f.cx + dx * ca - dy * sa).round().clamp(0.0, wmax) as u32;
            let y = (f.cy + dx * sa + dy * ca).round().clamp(0.0, hmax) as u32;
            let p = img.get_pixel(x, y);
            let at = (v * MESH_SIDE + u) * 3;
            for c in 0..3 {
                out[at + c] = f32::from(p[c]) / 255.0;
            }
        }
    }
    out
}

/// Rapport d'ouverture d'un œil sur ses six repères.
pub fn ear(landmarks: &[f32], idx: [usize; 6]) -> f32 {
    let p = |i: usize| (landmarks[idx[i] * 3], landmarks[idx[i] * 3 + 1]);
    let d = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let width = d(p(0), p(3));
    if width <= f32::EPSILON {
        return 1.0;
    }
    (d(p(1), p(5)) + d(p(2), p(4))) / (2.0 * width)
}

impl EyeDetector {
    /// Yeux fermés sur au moins un visage ? `None` si aucun visage n'est
    /// assez grand pour en juger, ou si un modèle échoue.
    pub fn eyes_closed(&mut self, img: &RgbImage) -> Option<bool> {
        let faces = detect(&mut self.detect, img)?;
        if faces.is_empty() {
            return None;
        }
        let mut any = false;
        for f in &faces {
            let out = run(&mut self.mesh, crop(img, f), [1, MESH_SIDE, MESH_SIDE, 3])?;
            let landmarks = out.iter().max_by_key(|o| o.len())?;
            if landmarks.len() < 478 * 3 {
                return None;
            }
            let (right, left) = (ear(landmarks, RIGHT_EYE), ear(landmarks, LEFT_EYE));
            any |= right < CLOSED_EAR && left < CLOSED_EAR;
        }
        Some(any)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapport_d_ouverture() {
        let mut lm = vec![0f32; 478 * 3];
        let mut put = |i: usize, x: f32, y: f32| {
            lm[i * 3] = x;
            lm[i * 3 + 1] = y;
        };
        // Œil de 10 de large, paupières à 1 au-dessus et au-dessous.
        let [a, b, c, d, e, f] = RIGHT_EYE;
        put(a, 0.0, 0.0);
        put(d, 10.0, 0.0);
        put(b, 3.0, -1.0);
        put(f, 3.0, 1.0);
        put(c, 7.0, -1.0);
        put(e, 7.0, 1.0);
        assert!((ear(&lm, RIGHT_EYE) - 0.2).abs() < 1e-6);
        // Œil dégénéré : jamais pris pour fermé.
        assert_eq!(ear(&lm, LEFT_EYE), 1.0);
    }

    #[test]
    fn portraits_libres() {
        let Some(detector) = detector() else {
            panic!(
                "ONNX Runtime absent : lancer « node scripts/fetch-onnxruntime.mjs » (décision 0010)"
            );
        };
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-assets");
        let load = |name: &str| image::open(assets.join(name)).unwrap().to_rgb8();
        let mut d = detector.lock().unwrap();
        assert_eq!(d.eyes_closed(&load("visage_ouverts.jpg")), Some(false));
        assert_eq!(d.eyes_closed(&load("visage_fermes.jpg")), Some(true));
        // Pas de visage : rien à dire.
        assert_eq!(
            d.eyes_closed(&RgbImage::from_pixel(320, 240, image::Rgb([90, 90, 90]))),
            None
        );
    }
}
