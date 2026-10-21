//! Détection des ratés (cahier des charges, 4.3), entièrement en local :
//! flou ou bougé, sous ou surexposition. Les yeux fermés viendront avec un
//! modèle ONNX, après prototype. Afterdusk signale, il ne rejette jamais.

pub mod run;

use image::imageops::{self, FilterType};
use image::{GrayImage, RgbImage};

/// Version de l'analyse : la changer relance l'analyse des photos déjà vues.
pub const VERSION: i64 = 1;

/// Côté long de l'image analysée : les mesures dépendent de l'échelle.
pub const ANALYSIS_SIDE: u32 = 1024;
/// Grille de tuiles pour la netteté.
const TILES: u32 = 8;
/// Tuiles les plus nettes retenues : le sujet, même petit, plutôt que le fond.
const BEST_TILES: usize = 4;

/// En dessous, la photo est probablement floue ou bougée. Calibré sur les
/// six RAW de test : nets de 290 à 3 400, floutés (rayon 3 px à 1 024 px)
/// de 6 à 74.
pub const BLUR_THRESHOLD: f32 = 80.0;
/// Part de pixels écrêtés (ou bouchés) au-delà de laquelle l'exposition est suspecte.
const CLIPPED_SHARE: f32 = 0.10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exposure {
    Under,
    Over,
}

impl Exposure {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Under => "under",
            Self::Over => "over",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Findings {
    pub sharpness: f32,
    pub blurry: bool,
    pub exposure: Option<Exposure>,
}

/// Luminance à l'échelle d'analyse.
pub fn prepare(img: &RgbImage) -> GrayImage {
    let gray = imageops::grayscale(img);
    let long = gray.width().max(gray.height());
    if long <= ANALYSIS_SIDE {
        return gray;
    }
    let scale = ANALYSIS_SIDE as f32 / long as f32;
    let w = ((gray.width() as f32 * scale).round() as u32).max(1);
    let h = ((gray.height() as f32 * scale).round() as u32).max(1);
    imageops::resize(&gray, w, h, FilterType::Triangle)
}

/// Netteté : variance du laplacien par tuile, moyenne des tuiles les plus
/// nettes. Un fond flou derrière un sujet net ne fait pas une photo floue ;
/// un bougé abaisse toutes les tuiles.
pub fn sharpness(gray: &GrayImage) -> f32 {
    let (w, h) = gray.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let px = |x: u32, y: u32| f32::from(gray.get_pixel(x, y)[0]);
    let tw = (w / TILES).max(3);
    let th = (h / TILES).max(3);
    let mut scores = Vec::new();
    for ty in 0..TILES {
        for tx in 0..TILES {
            let x0 = (tx * tw).max(1);
            let y0 = (ty * th).max(1);
            let x1 = ((tx + 1) * tw).min(w - 1);
            let y1 = ((ty + 1) * th).min(h - 1);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let (mut sum, mut sq, mut n) = (0.0f64, 0.0f64, 0.0f64);
            for y in y0..y1 {
                for x in x0..x1 {
                    let lap =
                        4.0 * px(x, y) - px(x - 1, y) - px(x + 1, y) - px(x, y - 1) - px(x, y + 1);
                    let lap = f64::from(lap);
                    sum += lap;
                    sq += lap * lap;
                    n += 1.0;
                }
            }
            if n > 0.0 {
                let mean = sum / n;
                scores.push((sq / n - mean * mean) as f32);
            }
        }
    }
    scores.sort_by(|a, b| b.total_cmp(a));
    let best = &scores[..scores.len().min(BEST_TILES)];
    if best.is_empty() {
        0.0
    } else {
        best.iter().sum::<f32>() / best.len() as f32
    }
}

/// Exposition : part de pixels écrêtés dans les hautes lumières ou bouchés
/// dans les ombres, et luminance moyenne.
pub fn exposure(gray: &GrayImage) -> Option<Exposure> {
    let total = (gray.width() * gray.height()) as f32;
    if total == 0.0 {
        return None;
    }
    let (mut high, mut low, mut sum) = (0u32, 0u32, 0u64);
    for p in gray.pixels() {
        let v = p[0];
        high += u32::from(v >= 250);
        low += u32::from(v <= 5);
        sum += u64::from(v);
    }
    let mean = sum as f32 / total;
    if high as f32 / total > CLIPPED_SHARE && mean > 150.0 || mean > 215.0 {
        Some(Exposure::Over)
    } else if low as f32 / total > 2.5 * CLIPPED_SHARE && mean < 60.0 || mean < 25.0 {
        Some(Exposure::Under)
    } else {
        None
    }
}

pub fn analyze(img: &RgbImage) -> Findings {
    let gray = prepare(img);
    let s = sharpness(&gray);
    Findings {
        sharpness: s,
        blurry: s < BLUR_THRESHOLD,
        exposure: exposure(&gray),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(side: u32, cell: u32) -> RgbImage {
        RgbImage::from_fn(side, side, |x, y| {
            let v = if (x / cell + y / cell).is_multiple_of(2) {
                40
            } else {
                210
            };
            image::Rgb([v, v, v])
        })
    }

    #[test]
    fn flou_et_fond_flou() {
        let sharp = checker(512, 8);
        let blurred = imageops::blur(&sharp, 6.0);
        let (s, b) = (sharpness(&prepare(&sharp)), sharpness(&prepare(&blurred)));
        assert!(s > 10.0 * b, "{s} {b}");
        assert!(analyze(&blurred).blurry);
        assert!(!analyze(&sharp).blurry);
        // Un petit sujet net sur un fond flou reste net.
        let mut portrait = blurred.clone();
        imageops::replace(
            &mut portrait,
            &imageops::crop_imm(&sharp, 0, 0, 96, 96).to_image(),
            200,
            200,
        );
        assert!(
            !analyze(&portrait).blurry,
            "{}",
            sharpness(&prepare(&portrait))
        );
    }

    #[test]
    fn echantillons_nets_et_floutes() {
        use crate::metadata::media::format_of;
        use crate::raw::preview;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-assets");
        for f in std::fs::read_dir(dir).unwrap() {
            let p = f.unwrap().path();
            let Some(fmt) = format_of(&p) else { continue };
            let img = preview::load(&p, fmt, 1, ANALYSIS_SIDE).unwrap();
            let a = analyze(&img);
            assert!(!a.blurry && a.exposure.is_none(), "{p:?} : {a:?}");
            assert!(analyze(&imageops::blur(&img, 3.0)).blurry, "{p:?} flouté");
        }
    }

    #[test]
    fn expositions() {
        let flat = |v: u8| RgbImage::from_pixel(64, 64, image::Rgb([v, v, v]));
        assert_eq!(exposure(&prepare(&flat(252))), Some(Exposure::Over));
        assert_eq!(exposure(&prepare(&flat(3))), Some(Exposure::Under));
        assert_eq!(exposure(&prepare(&flat(120))), None);
        assert_eq!(exposure(&prepare(&checker(64, 4))), None);
    }
}
