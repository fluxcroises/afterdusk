//! Lecture des RAW via LibRaw : métadonnées, EXIF et aperçu JPEG intégré.
//!
//! LibRaw ouvre les fichiers en lecture seule ; ce module n'écrit jamais
//! sur le fichier source.

use std::collections::BTreeMap;
use std::ffi::{CStr, CString, c_char};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

pub mod preview;

use afterdusk_libraw_sys as sys;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum RawError {
    #[error("chemin non valide : {0}")]
    InvalidPath(PathBuf),
    #[error("impossible d'ouvrir {path} : {message} (code LibRaw {code})")]
    Open {
        path: PathBuf,
        code: i32,
        message: String,
    },
    #[error("impossible d'extraire l'aperçu : {message} (code LibRaw {code})")]
    Preview { code: i32, message: String },
    #[error("aucun aperçu intégré")]
    NoPreview,
    #[error("format d'aperçu inconnu ({0})")]
    UnknownPreviewFormat(i32),
    #[error("impossible de développer le RAW : {message} (code LibRaw {code})")]
    Develop { code: i32, message: String },
}

/// Image développée par LibRaw : XYZ (D65) linéaire, 16 bits, trois canaux
/// entrelacés, orientation appliquée.
#[derive(Debug, Clone)]
pub struct DevelopedRaw {
    pub width: u32,
    pub height: u32,
    pub xyz: Vec<u16>,
    /// Blanc de la prise de vue (XYZ, Y = 1) : la lumière que la balance du
    /// boîtier a ramenée au blanc D65 de l'image.
    pub as_shot_white: [f64; 3],
}

/// Blanc D65 (XYZ, Y = 1).
pub const D65_WHITE: [f64; 3] = [0.950_47, 1.0, 1.088_83];

/// sRGB linéaire vers XYZ (D65).
const SRGB_TO_XYZ: [[f64; 3]; 3] = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175],
    [0.019_333_9, 0.119_192, 0.950_304_1],
];

/// Blanc de la prise de vue d'après les multiplicateurs du boîtier : le gris
/// neutre de la scène vaut 1 / cam_mul en valeurs du capteur, soit
/// pre_mul / cam_mul une fois ramené au neutre D65 que `rgb_cam` attend.
fn as_shot_white(c: &sys::ad_raw_color) -> [f64; 3] {
    let colors = usize::try_from(c.colors).unwrap_or(3).clamp(3, 4);
    let pick = |m: &[f32; 4], i: usize| {
        if i == 3 && m[3] <= 0.0 { m[1] } else { m[i] }
    };
    if (0..colors).any(|i| !(pick(&c.cam_mul, i) > 0.0 && pick(&c.pre_mul, i) > 0.0)) {
        return D65_WHITE;
    }
    let mut srgb = [0.0f64; 3];
    for (j, out) in srgb.iter_mut().enumerate() {
        for i in 0..colors {
            let normalized = f64::from(pick(&c.pre_mul, i)) / f64::from(pick(&c.cam_mul, i));
            *out += f64::from(c.rgb_cam[j][i]) * normalized;
        }
    }
    let xyz: [f64; 3] =
        std::array::from_fn(|r| (0..3).map(|k| SRGB_TO_XYZ[r][k] * srgb[k]).sum::<f64>());
    if xyz.iter().all(|v| v.is_finite() && *v > 0.0) {
        [xyz[0] / xyz[1], 1.0, xyz[2] / xyz[1]]
    } else {
        D65_WHITE
    }
}

/// Identifiant d'une balise TIFF/EXIF relevée dans le fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Ifd {
    Exif,
    Tiff,
    Gps,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Gps {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

/// Métadonnées utiles à Afterdusk (fiche d'informations, base, matériel).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RawInfo {
    pub make: String,
    pub model: String,
    pub lens_make: Option<String>,
    pub lens: Option<String>,
    pub body_serial: Option<String>,
    pub artist: Option<String>,
    pub iso: Option<f32>,
    /// Vitesse en secondes.
    pub shutter: Option<f32>,
    pub aperture: Option<f32>,
    pub focal_length: Option<f32>,
    pub focal_length_35mm: Option<u16>,
    pub exposure_compensation: Option<f32>,
    /// Date de prise de vue telle qu'écrite par le boîtier, sans fuseau
    /// (`AAAA-MM-JJTHH:MM:SS`).
    pub captured_at: Option<String>,
    /// Décalage horaire EXIF (`+02:00`) quand le boîtier l'écrit.
    pub utc_offset: Option<String>,
    pub width: u16,
    pub height: u16,
    /// Orientation EXIF (1 à 8).
    pub orientation: u8,
    /// Capteur Fujifilm X-Trans.
    pub is_xtrans: bool,
    pub is_dng: bool,
    pub gps: Option<Gps>,
    pub embedded_previews: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewFormat {
    Jpeg,
    /// RVB 8 bits non compressé.
    Bitmap,
    JpegXl,
}

/// Aperçu déclaré dans le fichier ; dimensions nulles quand le format ne les donne pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EmbeddedPreview {
    pub index: i32,
    pub is_jpeg: bool,
    pub width: u16,
    pub height: u16,
    pub length: u32,
}

#[derive(Debug, Clone)]
pub struct Preview {
    pub format: PreviewFormat,
    pub width: u16,
    pub height: u16,
    pub data: Vec<u8>,
}

/// Un fichier RAW ouvert par LibRaw.
pub struct RawFile {
    handle: NonNull<sys::ad_raw>,
}

// Un objet LibRaw peut changer de fil d'exécution mais pas être partagé.
unsafe impl Send for RawFile {}

impl RawFile {
    pub fn open(path: &Path) -> Result<Self, RawError> {
        let utf8 = path
            .to_str()
            .ok_or_else(|| RawError::InvalidPath(path.to_path_buf()))?;
        let c_path = CString::new(utf8).map_err(|_| RawError::InvalidPath(path.to_path_buf()))?;
        let mut code = 0;
        // SAFETY : c_path est une chaîne C valide pendant l'appel.
        let ptr = unsafe { sys::ad_raw_open(c_path.as_ptr(), &mut code) };
        match NonNull::new(ptr) {
            Some(handle) => Ok(Self { handle }),
            None => Err(RawError::Open {
                path: path.to_path_buf(),
                code,
                message: libraw_message(code),
            }),
        }
    }

    pub fn info(&self) -> RawInfo {
        // SAFETY : le pointeur reste valide tant que self existe.
        let i = unsafe { &*sys::ad_raw_get_info(self.handle.as_ptr()) };
        let exif = self.exif();
        let tag = |ifd, number| exif.get(&(ifd, number)).map(String::as_str);

        RawInfo {
            make: non_empty(&i.make)
                .or_else(|| non_empty(&i.normalized_make))
                .unwrap_or_default(),
            model: non_empty(&i.model)
                .or_else(|| non_empty(&i.normalized_model))
                .unwrap_or_default(),
            lens_make: non_empty(&i.lens_make),
            lens: non_empty(&i.lens),
            body_serial: non_empty(&i.body_serial)
                .or_else(|| tag(Ifd::Exif, 0xa431).map(str::to_owned)),
            artist: non_empty(&i.artist),
            iso: positive(i.iso_speed),
            shutter: positive(i.shutter),
            aperture: positive(i.aperture),
            focal_length: positive(i.focal_len),
            focal_length_35mm: (i.focal_len_35mm > 0).then_some(i.focal_len_35mm),
            exposure_compensation: tag(Ifd::Exif, 0x9204).and_then(parse_rational),
            captured_at: tag(Ifd::Exif, 0x9003)
                .or_else(|| tag(Ifd::Tiff, 0x0132))
                .and_then(parse_exif_datetime),
            utc_offset: tag(Ifd::Exif, 0x9011)
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_owned()),
            width: i.width,
            height: i.height,
            orientation: orientation_from_flip(i.flip),
            is_xtrans: i.filters == 9,
            is_dng: i.dng_version != 0,
            gps: gps(i),
            embedded_previews: u32::try_from(i.thumb_count).unwrap_or(0),
        }
    }

    /// Balises EXIF, TIFF (IFD0) et GPS relevées par LibRaw, valeurs en texte.
    pub fn exif(&self) -> BTreeMap<(Ifd, u16), String> {
        let raw = self.handle.as_ptr();
        // SAFETY : le pointeur reste valide tant que self existe.
        let count = unsafe { sys::ad_raw_exif_count(raw) };
        let mut tags = BTreeMap::new();
        for index in 0..count {
            let (mut ifd, mut tag) = (0u32, 0u32);
            let mut value: *const c_char = std::ptr::null();
            // SAFETY : index < count ; value pointe dans la mémoire de LibRaw.
            let found = unsafe { sys::ad_raw_exif_get(raw, index, &mut ifd, &mut tag, &mut value) };
            if found == 0 || value.is_null() {
                continue;
            }
            let ifd = match ifd {
                sys::AD_IFD_EXIF => Ifd::Exif,
                sys::AD_IFD_TIFF0 => Ifd::Tiff,
                sys::AD_IFD_GPS => Ifd::Gps,
                _ => continue,
            };
            // SAFETY : chaîne terminée par un zéro, copiée immédiatement.
            let text = unsafe { CStr::from_ptr(value) }
                .to_string_lossy()
                .into_owned();
            if let Ok(tag) = u16::try_from(tag) {
                tags.insert((ifd, tag), text);
            }
        }
        tags
    }

    /// Aperçus intégrés déclarés par le fichier, sans les extraire.
    pub fn embedded_previews(&self) -> Vec<EmbeddedPreview> {
        let raw = self.handle.as_ptr();
        // SAFETY : le pointeur reste valide tant que self existe.
        let count = unsafe { sys::ad_raw_thumb_count(raw) };
        (0..count)
            .filter_map(|index| {
                let (mut format, mut width, mut height, mut length) = (0, 0u16, 0u16, 0u32);
                // SAFETY : index < count, pointeurs de sortie valides.
                let found = unsafe {
                    sys::ad_raw_thumb_item(
                        raw,
                        index,
                        &mut format,
                        &mut width,
                        &mut height,
                        &mut length,
                    )
                };
                (found != 0).then_some(EmbeddedPreview {
                    index,
                    is_jpeg: format == sys::AD_INTERNAL_THUMB_JPEG,
                    width,
                    height,
                    length,
                })
            })
            .collect()
    }

    /// Développe l'image sans retouche (voir `ad_raw_develop`) ; `half_size` :
    /// moitié de la définition, pour l'aperçu de retouche. Lecture seule.
    pub fn develop(&mut self, half_size: bool) -> Result<DevelopedRaw, RawError> {
        let mut data: *const u16 = std::ptr::null();
        let (mut width, mut height) = (0u16, 0u16);
        // SAFETY : pointeurs de sortie valides ; LibRaw garantit
        // largeur × hauteur × 3 valeurs lisibles jusqu'au prochain appel.
        let code = unsafe {
            sys::ad_raw_develop(
                self.handle.as_ptr(),
                i32::from(half_size),
                &mut data,
                &mut width,
                &mut height,
            )
        };
        if code != 0 || data.is_null() {
            return Err(RawError::Develop {
                code,
                message: libraw_message(code),
            });
        }
        let len = usize::from(width) * usize::from(height) * 3;
        let xyz = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
        // SAFETY : la structure vit aussi longtemps que le fichier ouvert.
        let color = unsafe { sys::ad_raw_get_color(self.handle.as_ptr()).as_ref() };
        Ok(DevelopedRaw {
            width: u32::from(width),
            height: u32::from(height),
            xyz,
            as_shot_white: color.map_or(D65_WHITE, as_shot_white),
        })
    }

    /// Extrait le plus grand aperçu intégré, en privilégiant le JPEG.
    pub fn preview(&mut self) -> Result<Preview, RawError> {
        let best = self
            .embedded_previews()
            .into_iter()
            // Les dimensions déclarées manquent parfois (aperçu pleine taille des
            // CR3) : la taille en octets est un meilleur indicateur.
            .max_by_key(|p| (p.is_jpeg, p.length))
            .ok_or(RawError::NoPreview)?;
        self.preview_at(best.index)
    }

    /// Extrait le plus petit aperçu JPEG dont le côté long atteint `min_side`,
    /// ou à défaut le plus grand : une miniature n'a pas besoin du plein format.
    pub fn preview_for(&mut self, min_side: u32) -> Result<Preview, RawError> {
        let fitting = self
            .embedded_previews()
            .into_iter()
            .filter(|p| p.is_jpeg && u32::from(p.width.max(p.height)) >= min_side)
            .min_by_key(|p| p.length);
        match fitting {
            Some(p) => self.preview_at(p.index),
            None => self.preview(),
        }
    }

    /// Extrait l'aperçu `index` de [`RawFile::embedded_previews`].
    pub fn preview_at(&mut self, index: i32) -> Result<Preview, RawError> {
        let mut data: *const u8 = std::ptr::null();
        let (mut size, mut format) = (0usize, 0);
        let (mut width, mut height, mut colors, mut bits) = (0u16, 0u16, 0u16, 0u16);
        // SAFETY : tous les pointeurs de sortie sont valides.
        let code = unsafe {
            sys::ad_raw_thumbnail(
                self.handle.as_ptr(),
                index,
                &mut data,
                &mut size,
                &mut format,
                &mut width,
                &mut height,
                &mut colors,
                &mut bits,
            )
        };
        if code != 0 || data.is_null() {
            return Err(RawError::Preview {
                code,
                message: libraw_message(code),
            });
        }
        let format = match format {
            sys::AD_THUMB_JPEG => PreviewFormat::Jpeg,
            sys::AD_THUMB_BITMAP => PreviewFormat::Bitmap,
            sys::AD_THUMB_JPEGXL => PreviewFormat::JpegXl,
            other => return Err(RawError::UnknownPreviewFormat(other)),
        };
        // SAFETY : LibRaw garantit `size` octets lisibles jusqu'au prochain appel.
        let bytes = unsafe { std::slice::from_raw_parts(data, size) }.to_vec();
        if format == PreviewFormat::Jpeg {
            // Les dimensions déclarées peuvent manquer ou mentir : le JPEG fait foi.
            if let Some((w, h)) = jpeg_dimensions(&bytes) {
                (width, height) = (w, h);
            }
        }
        Ok(Preview {
            format,
            width,
            height,
            data: bytes,
        })
    }
}

/// Lit largeur et hauteur dans l'en-tête SOF d'un JPEG.
fn jpeg_dimensions(data: &[u8]) -> Option<(u16, u16)> {
    if !data.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut i = 2;
    while i + 9 < data.len() {
        if data[i] != 0xff {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        match marker {
            0xff => i += 1,
            0x01 | 0xd0..=0xd8 => i += 2,
            0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => {
                let height = u16::from_be_bytes([data[i + 5], data[i + 6]]);
                let width = u16::from_be_bytes([data[i + 7], data[i + 8]]);
                return Some((width, height));
            }
            _ => {
                let length = usize::from(u16::from_be_bytes([data[i + 2], data[i + 3]]));
                i += 2 + length;
            }
        }
    }
    None
}

impl Drop for RawFile {
    fn drop(&mut self) {
        // SAFETY : le pointeur vient de ad_raw_open et n'est libéré qu'ici.
        unsafe { sys::ad_raw_close(self.handle.as_ptr()) }
    }
}

/// Version de LibRaw compilée dans Afterdusk.
pub fn libraw_version() -> String {
    // SAFETY : chaîne statique de LibRaw.
    unsafe { CStr::from_ptr(sys::ad_raw_version()) }
        .to_string_lossy()
        .into_owned()
}

fn libraw_message(code: i32) -> String {
    // SAFETY : libraw_strerror renvoie toujours une chaîne statique.
    unsafe { CStr::from_ptr(sys::ad_raw_strerror(code)) }
        .to_string_lossy()
        .into_owned()
}

fn non_empty(chars: &[c_char]) -> Option<String> {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    let text = String::from_utf8_lossy(&bytes).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn positive(value: f32) -> Option<f32> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn parse_rational(text: &str) -> Option<f32> {
    let (num, den) = text.split_once('/')?;
    let (num, den): (f32, f32) = (num.trim().parse().ok()?, den.trim().parse().ok()?);
    (den != 0.0).then(|| num / den)
}

/// `AAAA:MM:JJ HH:MM:SS` → `AAAA-MM-JJTHH:MM:SS`.
fn parse_exif_datetime(text: &str) -> Option<String> {
    let text = text.trim();
    let b = text.as_bytes();
    let well_formed = b.len() >= 19
        && b[4] == b':'
        && b[7] == b':'
        && b[13] == b':'
        && b[16] == b':'
        && b[..19]
            .iter()
            .enumerate()
            .all(|(k, c)| matches!(k, 4 | 7 | 10 | 13 | 16) || c.is_ascii_digit());
    if !well_formed || text.starts_with("0000") {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}",
        &text[0..4],
        &text[5..7],
        &text[8..10],
        &text[11..19]
    ))
}

/// Convertit le `flip` de LibRaw en orientation EXIF.
fn orientation_from_flip(flip: i32) -> u8 {
    match flip {
        3 => 3,
        5 => 8,
        6 => 6,
        _ => 1,
    }
}

fn gps(i: &sys::ad_raw_info) -> Option<Gps> {
    if i.gps_parsed == 0 {
        return None;
    }
    let dms = |v: [f32; 3]| f64::from(v[0]) + f64::from(v[1]) / 60.0 + f64::from(v[2]) / 3600.0;
    let mut latitude = dms(i.gps_latitude);
    let mut longitude = dms(i.gps_longitude);
    if latitude == 0.0 && longitude == 0.0 {
        return None;
    }
    if i.gps_latref as u8 == b'S' {
        latitude = -latitude;
    }
    if i.gps_longref as u8 == b'W' {
        longitude = -longitude;
    }
    let altitude = (i.gps_altitude != 0.0).then(|| {
        let alt = f64::from(i.gps_altitude);
        if i.gps_altref == 1 { -alt } else { alt }
    });
    Some(Gps {
        latitude,
        longitude,
        altitude,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_exif_convertie_en_iso() {
        assert_eq!(
            parse_exif_datetime("2026:10:18 21:04:59").as_deref(),
            Some("2026-10-18T21:04:59")
        );
        assert_eq!(parse_exif_datetime("0000:00:00 00:00:00"), None);
        assert_eq!(parse_exif_datetime("n'importe quoi"), None);
    }

    #[test]
    fn rationnel_signe() {
        assert_eq!(
            parse_rational("-2/3").map(|v| (v * 3.0).round()),
            Some(-2.0)
        );
        assert_eq!(parse_rational("1/0"), None);
    }

    #[test]
    fn dimensions_jpeg() {
        // SOI, APP0 de 4 octets, SOF0 1620x1080.
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x04,
            0x38, 0x06, 0x54, 0x03, 0x00, 0x00,
        ];
        assert_eq!(jpeg_dimensions(&jpeg), Some((1620, 1080)));
        assert_eq!(jpeg_dimensions(b"pas un jpeg"), None);
    }

    #[test]
    fn orientation() {
        assert_eq!(orientation_from_flip(0), 1);
        assert_eq!(orientation_from_flip(6), 6);
        assert_eq!(orientation_from_flip(5), 8);
    }
}
