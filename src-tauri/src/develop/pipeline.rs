//! Pipeline de développement, référence processeur (décision 0005) : chaque
//! étape a un jumeau WGSL (`gpu.rs`) qui doit donner le même résultat, à une
//! tolérance près vérifiée par les tests.
//!
//! Image de travail : linéaire, Rec. 2020, virgule flottante, 1.0 = blanc du
//! capteur. Rendu : exposition, courbe de tonalité neutre, sRGB 8 bits.

use std::thread;

use super::settings::DevelopSettings;
use crate::raw::DevelopedRaw;

/// Image de travail : RVB linéaire Rec. 2020 entrelacé.
#[derive(Debug, Clone)]
pub struct WorkingImage {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<f32>,
}

/// XYZ (D65) vers Rec. 2020 linéaire.
pub const XYZ_TO_REC2020: [[f32; 3]; 3] = [
    [1.716_651, -0.355_670_8, -0.253_366_3],
    [-0.666_684_4, 1.616_481, 0.015_768_5],
    [0.017_639_9, -0.042_770_6, 0.942_103_1],
];

/// Rec. 2020 linéaire vers sRGB linéaire (même blanc D65).
pub const REC2020_TO_SRGB: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641_1, -0.072_849_9],
    [-0.124_550_5, 1.132_9, -0.008_349_4],
    [-0.018_150_8, -0.100_578_9, 1.118_73],
];

/// Paramètres figés du processus 1 : ce que « neutre » veut dire.
pub mod process1 {
    /// Décalage d'exposition de base : les boîtiers placent le gris moyen
    /// plus bas dans le RAW que dans leur JPEG. Mesuré sur les six formats de
    /// test-assets : à +1 IL, la luminance moyenne du rendu neutre égale celle
    /// du JPEG du boîtier (moyenne géométrique des rapports 1,00 ; de 0,88 à 1,20).
    pub const BASELINE_EV: f32 = 1.0;
    /// Début de l'épaule de la courbe : au-dessus, les hautes lumières sont
    /// comprimées en douceur vers le blanc au lieu d'être écrêtées.
    pub const SHOULDER: f32 = 0.6;
}

#[inline]
fn mul(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// Courbe neutre du processus 1 : identité jusqu'à l'épaule, puis
/// compression exponentielle (continue, de pente 1 à la jonction) vers 1.
#[inline]
pub fn neutral_curve(x: f32) -> f32 {
    let t = process1::SHOULDER;
    if x <= t {
        x.max(0.0)
    } else {
        t + (1.0 - t) * (1.0 - (-(x - t) / (1.0 - t)).exp())
    }
}

/// Fonction de transfert sRGB (linéaire → codé).
#[inline]
pub fn srgb_encode(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

impl WorkingImage {
    /// Depuis le RAW développé par LibRaw, réduit pour que le côté long ne
    /// dépasse pas `max_side` (moyenne par blocs, en linéaire).
    pub fn from_raw(raw: &DevelopedRaw, max_side: u32) -> Self {
        let (w, h) = (raw.width as usize, raw.height as usize);
        let long = w.max(h).max(1);
        let factor = long.div_ceil(max_side.max(1) as usize).max(1);
        let (ow, oh) = (w / factor, h / factor);
        let mut rgb = vec![0.0f32; ow * oh * 3];
        let scale = 1.0 / (65535.0 * (factor * factor) as f32);
        for oy in 0..oh {
            for ox in 0..ow {
                let mut acc = [0.0f32; 3];
                for dy in 0..factor {
                    let row = (oy * factor + dy) * w;
                    for dx in 0..factor {
                        let i = (row + ox * factor + dx) * 3;
                        acc[0] += f32::from(raw.xyz[i]);
                        acc[1] += f32::from(raw.xyz[i + 1]);
                        acc[2] += f32::from(raw.xyz[i + 2]);
                    }
                }
                let xyz = [acc[0] * scale, acc[1] * scale, acc[2] * scale];
                let out = mul(&XYZ_TO_REC2020, xyz);
                let o = (oy * ow + ox) * 3;
                rgb[o..o + 3].copy_from_slice(&out);
            }
        }
        Self {
            width: ow as u32,
            height: oh as u32,
            rgb,
        }
    }
}

/// Paramètres de rendu dérivés des réglages, communs au processeur et au GPU.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderParams {
    /// Multiplicateur linéaire : 2^(exposition + décalage de base).
    pub gain: f32,
}

impl RenderParams {
    pub fn from_settings(settings: &DevelopSettings) -> Self {
        // Processus 1 seulement pour l'instant ; un processus 2 aurait ses propres constantes.
        let ev = settings.light.exposure as f32 + process1::BASELINE_EV;
        Self { gain: ev.exp2() }
    }
}

/// Rendu d'un pixel de travail en sRGB codé (0 à 1).
#[inline]
pub fn render_pixel(rgb: [f32; 3], p: &RenderParams) -> [f32; 3] {
    let lin = [rgb[0] * p.gain, rgb[1] * p.gain, rgb[2] * p.gain];
    let toned = [
        neutral_curve(lin[0]),
        neutral_curve(lin[1]),
        neutral_curve(lin[2]),
    ];
    let srgb = mul(&REC2020_TO_SRGB, toned);
    [
        srgb_encode(srgb[0]),
        srgb_encode(srgb[1]),
        srgb_encode(srgb[2]),
    ]
}

/// Rendu complet en RGBA 8 bits, réparti sur les cœurs du processeur.
pub fn render_rgba(image: &WorkingImage, settings: &DevelopSettings) -> Vec<u8> {
    let params = RenderParams::from_settings(settings);
    let pixels = (image.width * image.height) as usize;
    let mut out = vec![0u8; pixels * 4];
    let workers = thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = pixels.div_ceil(workers).max(1);
    thread::scope(|scope| {
        for (n, slice) in out.chunks_mut(chunk * 4).enumerate() {
            let src = &image.rgb[n * chunk * 3..];
            scope.spawn(move || {
                for (i, px) in slice.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let v = render_pixel([src[i * 3], src[i * 3 + 1], src[i * 3 + 2]], &params);
                    px[0] = (v[0] * 255.0 + 0.5) as u8;
                    px[1] = (v[1] * 255.0 + 0.5) as u8;
                    px[2] = (v[2] * 255.0 + 0.5) as u8;
                    px[3] = 255;
                }
            });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courbe_neutre_continue_et_bornee() {
        let t = process1::SHOULDER;
        assert_eq!(neutral_curve(0.18), 0.18);
        assert!((neutral_curve(t + 1e-4) - neutral_curve(t)).abs() < 2e-4);
        assert!(neutral_curve(100.0) <= 1.0);
        assert!(neutral_curve(2.0) > neutral_curve(1.0));
        assert_eq!(neutral_curve(-1.0), 0.0);
    }

    #[test]
    fn blanc_et_gris_neutres() {
        // Un gris neutre reste neutre : les matrices partagent le blanc D65.
        let p = RenderParams { gain: 1.0 };
        let grey = mul(&XYZ_TO_REC2020, [0.950_47 * 0.18, 0.18, 1.088_83 * 0.18]);
        let out = render_pixel(grey, &p);
        assert!(
            (out[0] - out[1]).abs() < 2e-3 && (out[1] - out[2]).abs() < 2e-3,
            "{out:?}"
        );
        // 18 % de gris : environ 118/255 en sRGB.
        assert!((out[1] * 255.0 - 118.0).abs() < 2.0, "{out:?}");
    }
}
