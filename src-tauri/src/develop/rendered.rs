//! Photos déjà développées (JPEG, TIFF, PNG, HEIF) comme source de la
//! retouche : décodées, ramenées en sRGB selon leur profil ICC, orientées,
//! puis passées en XYZ linéaire comme la sortie de LibRaw. Leur balance des
//! blancs est déjà faite : le blanc de prise de vue est D65, et la balance
//! se règle en décalage relatif. L'original n'est qu'ouvert en lecture.

use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageReader};
use moxcms::{ColorProfile, Layout, TransformOptions};

use crate::raw::{D65_WHITE, DevelopedRaw};

#[derive(Debug, thiserror::Error)]
pub enum RenderedError {
    #[error("image illisible : {0}")]
    Decode(String),
    #[error("HEIF illisible (FFmpeg absent ou fichier non pris en charge) : {0}")]
    Heif(String),
}

/// sRGB linéaire vers XYZ (blanc D65).
const SRGB_TO_XYZ: [[f32; 3]; 3] = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175],
    [0.019_333_9, 0.119_192, 0.950_304_1],
];

/// Fonction de transfert sRGB inverse (codé → linéaire).
fn srgb_decode(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn is_heif(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|e| matches!(e.as_str(), "heic" | "heif" | "hif"))
}

/// Pixels sRGB codés (0 à 1) et profil ICC éventuel, depuis un fichier ou
/// des octets (HEIF passé par FFmpeg).
fn decode_any(path: &Path) -> Result<(DynamicImage, Option<Vec<u8>>), RenderedError> {
    let err = |e: image::ImageError| RenderedError::Decode(format!("{} : {e}", path.display()));
    if is_heif(path) {
        let png = crate::video::still(path)
            .ok_or_else(|| RenderedError::Heif(path.display().to_string()))?;
        let mut decoder =
            image::codecs::png::PngDecoder::new(std::io::Cursor::new(png)).map_err(err)?;
        let icc = decoder.icc_profile().ok().flatten();
        return Ok((DynamicImage::from_decoder(decoder).map_err(err)?, icc));
    }
    let reader = ImageReader::open(path)
        .map_err(|e| RenderedError::Decode(format!("{} : {e}", path.display())))?
        .with_guessed_format()
        .map_err(|e| RenderedError::Decode(format!("{} : {e}", path.display())))?;
    let mut decoder = reader.into_decoder().map_err(err)?;
    let icc = decoder.icc_profile().ok().flatten();
    Ok((DynamicImage::from_decoder(decoder).map_err(err)?, icc))
}

fn orient(img: DynamicImage, orientation: u8) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// Ramène des valeurs codées selon `icc` vers le sRGB codé. Un profil
/// illisible est traité comme du sRGB. Les couleurs hors du sRGB sont
/// ramenées à sa limite.
fn to_srgb(rgb: &mut [f32], icc: Option<&[u8]>) {
    let Some(source) = icc.and_then(|b| ColorProfile::new_from_slice(b).ok()) else {
        return;
    };
    let Ok(transform) = source.create_transform_f32(
        Layout::Rgb,
        &ColorProfile::new_srgb(),
        Layout::Rgb,
        TransformOptions::default(),
    ) else {
        return;
    };
    let src = rgb.to_vec();
    if transform.transform(&src, rgb).is_err() {
        rgb.copy_from_slice(&src);
    }
}

/// Décode une photo déjà développée en XYZ linéaire (0 à 65535), orientée
/// selon `orientation` (EXIF, 1 à 8).
pub fn develop(path: &Path, orientation: u8) -> Result<DevelopedRaw, RenderedError> {
    // HEIF : FFmpeg l'a déjà tourné (voir `metadata::media::read_image`) ;
    // une ancienne base peut encore porter l'Orientation de l'EXIF.
    let orientation = if is_heif(path) { 1 } else { orientation };
    let (img, icc) = decode_any(path)?;
    let img = orient(img, orientation).into_rgb32f();
    let (width, height) = (img.width(), img.height());
    let mut rgb = img.into_raw();
    to_srgb(&mut rgb, icc.as_deref());
    let xyz = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|p| {
            let lin = [srgb_decode(p[0]), srgb_decode(p[1]), srgb_decode(p[2])];
            SRGB_TO_XYZ.map(|row| {
                let v = row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2];
                (v.clamp(0.0, 1.0) * 65535.0).round() as u16
            })
        })
        .collect();
    Ok(DevelopedRaw {
        width,
        height,
        xyz,
        as_shot_white: D65_WHITE,
        full_long_side: width.max(height),
        rendered: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::pipeline::{Clipping, RenderParams, WorkingImage, render_encoded};
    use crate::develop::settings::DevelopSettings;

    /// Réglages à zéro : le rendu redonne le JPEG (à l'arrondi près).
    #[test]
    fn rendu_neutre_identique_a_l_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.png");
        let src = image::RgbImage::from_fn(40, 30, |x, y| {
            image::Rgb([(x * 6) as u8, (y * 8) as u8, ((x + y) * 3) as u8])
        });
        src.save(&path).unwrap();
        let developed = develop(&path, 1).unwrap();
        assert!(developed.rendered);
        let image = WorkingImage::from_raw(&developed, 2048);
        let params = RenderParams::from_settings(
            &DevelopSettings::default(),
            image.as_shot_white,
            Clipping::default(),
        )
        .rendered(image.rendered);
        let out = render_encoded(&image, &params);
        let worst = out
            .iter()
            .zip(src.as_raw())
            .map(|(a, b)| (a * 255.0 - f32::from(*b)).abs())
            .fold(0.0f32, f32::max);
        assert!(worst <= 1.5, "écart maximal {worst}");
    }

    #[test]
    fn orientation_appliquee() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.png");
        image::RgbImage::new(40, 30).save(&path).unwrap();
        let d = develop(&path, 6).unwrap();
        assert_eq!((d.width, d.height), (30, 40));
    }

    /// HEIF écrit par l'encodeur du système, relu par FFmpeg : dimensions et
    /// couleurs gardées. Sauté sans encodeur HEIF ou sans FFmpeg.
    #[test]
    fn heif_lu_par_ffmpeg() {
        if !crate::export::heif::available() || !crate::video::available() {
            eprintln!("encodeur HEIF ou FFmpeg absent : test sauté");
            return;
        }
        let w = 64;
        let h = 48;
        let img = crate::export::render::Rendered {
            width: w,
            height: h,
            rgb: (0..w * h).flat_map(|_| [0.8f32, 0.3, 0.2]).collect(),
        };
        let icc = crate::export::color::ColorSpace::Srgb.icc().unwrap();
        let bytes = crate::export::heif::encode(
            &img,
            95,
            &icc,
            &crate::export::encode::Embedded::default(),
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.heic");
        std::fs::write(&path, bytes).unwrap();
        let d = develop(&path, 1).unwrap();
        assert_eq!((d.width, d.height), (w, h));
        // Orientation de l'EXIF (iPhone : 6) ignorée, FFmpeg ayant déjà
        // appliqué la rotation du conteneur : pas de quart de tour en plus.
        let again = develop(&path, 6).unwrap();
        assert_eq!((again.width, again.height), (w, h));
        // Rouge dominant (X > Z) au milieu de l'image.
        let mid = ((h / 2 * w + w / 2) * 3) as usize;
        assert!(d.xyz[mid] > d.xyz[mid + 2], "{:?}", &d.xyz[mid..mid + 3]);
    }
}
