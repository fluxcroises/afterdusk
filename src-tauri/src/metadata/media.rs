//! Métadonnées d'un média, quel que soit son format : LibRaw pour les RAW,
//! kamadak-exif pour les JPEG, HEIF et TIFF.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{In, Reader, Tag, Value};
use serde::Serialize;

use crate::raw::{Gps, Ifd, RawFile};

/// Extensions RAW reconnues (en minuscules).
pub const RAW_EXTENSIONS: &[&str] = &[
    "3fr", "arw", "cr2", "cr3", "crw", "dcr", "dng", "erf", "fff", "iiq", "k25", "kdc", "mef",
    "mos", "mrw", "nef", "nrw", "orf", "ori", "pef", "raf", "raw", "rw2", "rwl", "sr2", "srf",
    "srw", "x3f",
];

/// Autres images importées en V0.1.
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "heic", "heif", "hif", "tif", "tiff", "png"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaFormat {
    Raw,
    Image,
}

pub fn format_of(path: &Path) -> Option<MediaFormat> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if RAW_EXTENSIONS.contains(&ext.as_str()) {
        Some(MediaFormat::Raw)
    } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some(MediaFormat::Image)
    } else {
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub body_serial: Option<String>,
    pub iso: Option<f32>,
    pub shutter: Option<f32>,
    pub aperture: Option<f32>,
    pub focal_length: Option<f32>,
    pub focal_length_35mm: Option<u16>,
    pub exposure_compensation: Option<f32>,
    pub exposure_program: Option<u32>,
    pub flash: Option<u32>,
    /// `AAAA-MM-JJTHH:MM:SS`, heure du boîtier.
    pub captured_at: Option<String>,
    pub utc_offset: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub orientation: u8,
    pub gps: Option<Gps>,
    /// Balises relevées, clé `ifd.0xTAG`.
    #[serde(skip)]
    pub tags: BTreeMap<String, String>,
}

impl MediaMetadata {
    /// Toutes les balises, pour la colonne `media_exif.raw_json`.
    pub fn tags_json(&self) -> String {
        serde_json::to_string(&self.tags).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Nom du boîtier pour les modèles de renommage (« X-T3 », « EOS R6 »).
    pub fn camera_label(&self) -> String {
        match (&self.make, &self.model) {
            (_, Some(model)) => model.clone(),
            (Some(make), None) => make.clone(),
            _ => String::new(),
        }
    }
}

/// Lit les métadonnées ; un fichier illisible donne des métadonnées vides
/// plutôt qu'une erreur, pour que l'import reste possible.
pub fn read_metadata(path: &Path, format: MediaFormat) -> MediaMetadata {
    match format {
        MediaFormat::Raw => read_raw(path),
        MediaFormat::Image => read_image(path),
    }
    .unwrap_or_default()
}

fn read_raw(path: &Path) -> Option<MediaMetadata> {
    let raw = RawFile::open(path).ok()?;
    let info = raw.info();
    let exif = raw.exif();
    let tag = |ifd, number| exif.get(&(ifd, number));
    let uint = |ifd, number| tag(ifd, number).and_then(|v| v.split(' ').next()?.parse().ok());
    let mut tags = BTreeMap::new();
    for ((ifd, number), value) in &exif {
        let prefix = match ifd {
            Ifd::Exif => "exif",
            Ifd::Tiff => "tiff",
            Ifd::Gps => "gps",
        };
        tags.insert(format!("{prefix}.0x{number:04x}"), value.clone());
    }
    Some(MediaMetadata {
        make: (!info.make.is_empty()).then_some(info.make),
        model: (!info.model.is_empty()).then_some(info.model),
        lens: info.lens,
        body_serial: info.body_serial,
        iso: info.iso,
        shutter: info.shutter,
        aperture: info.aperture,
        focal_length: info.focal_length,
        focal_length_35mm: info.focal_length_35mm,
        exposure_compensation: info.exposure_compensation,
        exposure_program: uint(Ifd::Exif, 0x8822),
        flash: uint(Ifd::Exif, 0x9209),
        captured_at: info.captured_at,
        utc_offset: info.utc_offset,
        width: Some(u32::from(info.width)),
        height: Some(u32::from(info.height)),
        orientation: info.orientation,
        gps: info.gps,
        tags,
    })
}

fn ascii(value: &Value) -> Option<String> {
    match value {
        Value::Ascii(parts) => parts
            .first()
            .map(|p| String::from_utf8_lossy(p).trim().to_owned())
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn rational(value: &Value) -> Option<f32> {
    match value {
        Value::Rational(v) => v
            .first()
            .filter(|r| r.denom != 0)
            .map(|r| r.to_f64() as f32),
        Value::SRational(v) => v
            .first()
            .filter(|r| r.denom != 0)
            .map(|r| r.to_f64() as f32),
        _ => value.get_uint(0).map(|u| u as f32),
    }
}

fn read_image(path: &Path) -> Option<MediaMetadata> {
    let file = File::open(path).ok()?;
    let exif = Reader::new()
        .read_from_container(&mut BufReader::new(file))
        .ok()?;
    let field = |tag| exif.get_field(tag, In::PRIMARY).map(|f| &f.value);
    let text = |tag| field(tag).and_then(ascii);
    let num = |tag| field(tag).and_then(rational);
    let uint = |tag| field(tag).and_then(|v| v.get_uint(0));

    let mut tags = BTreeMap::new();
    for f in exif.fields().filter(|f| f.ifd_num == In::PRIMARY) {
        let prefix = match f.tag.context() {
            exif::Context::Exif => "exif",
            exif::Context::Gps => "gps",
            _ => "tiff",
        };
        tags.insert(
            format!("{prefix}.0x{:04x}", f.tag.number()),
            f.display_value().to_string(),
        );
    }

    let captured_at = text(Tag::DateTimeOriginal)
        .or_else(|| text(Tag::DateTime))
        .and_then(|s| exif_datetime(&s));

    Some(MediaMetadata {
        make: text(Tag::Make),
        model: text(Tag::Model),
        lens: text(Tag::LensModel),
        body_serial: text(Tag::BodySerialNumber),
        iso: uint(Tag::PhotographicSensitivity).map(|v| v as f32),
        shutter: num(Tag::ExposureTime),
        aperture: num(Tag::FNumber),
        focal_length: num(Tag::FocalLength),
        focal_length_35mm: uint(Tag::FocalLengthIn35mmFilm).and_then(|v| u16::try_from(v).ok()),
        exposure_compensation: num(Tag::ExposureBiasValue),
        exposure_program: uint(Tag::ExposureProgram),
        flash: uint(Tag::Flash),
        captured_at,
        utc_offset: text(Tag::OffsetTimeOriginal),
        width: uint(Tag::PixelXDimension).or_else(|| uint(Tag::ImageWidth)),
        height: uint(Tag::PixelYDimension).or_else(|| uint(Tag::ImageLength)),
        orientation: uint(Tag::Orientation)
            .and_then(|v| u8::try_from(v).ok())
            .filter(|v| (1..=8).contains(v))
            .unwrap_or(1),
        gps: None,
        tags,
    })
}

/// `AAAA:MM:JJ HH:MM:SS` → `AAAA-MM-JJTHH:MM:SS`.
fn exif_datetime(text: &str) -> Option<String> {
    let b = text.as_bytes();
    if b.len() < 19 || text.starts_with("0000") {
        return None;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    if !digits.iter().all(|&i| b[i].is_ascii_digit()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_reconnus() {
        assert_eq!(
            format_of(Path::new("a/DSCF0001.RAF")),
            Some(MediaFormat::Raw)
        );
        assert_eq!(format_of(Path::new("IMG_0001.cr3")), Some(MediaFormat::Raw));
        assert_eq!(
            format_of(Path::new("IMG_0001.JPG")),
            Some(MediaFormat::Image)
        );
        assert_eq!(format_of(Path::new("MVI_0001.MP4")), None);
        assert_eq!(format_of(Path::new("SANS_EXTENSION")), None);
    }

    #[test]
    fn fichier_illisible_donne_des_metadonnees_vides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("abime.NEF");
        std::fs::write(&path, b"pas un raw").unwrap();
        assert_eq!(
            read_metadata(&path, MediaFormat::Raw),
            MediaMetadata::default()
        );
    }

    #[test]
    fn date_exif() {
        assert_eq!(
            exif_datetime("2026:10:18 21:04:59").as_deref(),
            Some("2026-10-18T21:04:59")
        );
        assert_eq!(exif_datetime("    :  :     :  :  "), None);
    }
}
