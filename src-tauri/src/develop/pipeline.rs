//! Pipeline de développement, référence processeur (décision 0005) : chaque
//! étape a un jumeau WGSL (`gpu.rs`) qui doit donner le même résultat, à une
//! tolérance près vérifiée par les tests.
//!
//! Image de travail : linéaire, Rec. 2020, virgule flottante, 1.0 = blanc du
//! capteur, à la balance du boîtier. Rendu : exposition et balance des blancs
//! (une matrice), courbe neutre, tonalité, couleur, sRGB 8 bits, histogramme
//! et avertissements d'écrêtage.

use std::thread;

use serde::Deserialize;

use super::settings::DevelopSettings;
use super::white;
#[cfg(test)]
use crate::raw::D65_WHITE;
use crate::raw::DevelopedRaw;

/// Image de travail : RVB linéaire Rec. 2020 entrelacé.
#[derive(Debug, Clone)]
pub struct WorkingImage {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<f32>,
    /// Blanc de la prise de vue (XYZ, Y = 1), pour changer de balance.
    pub as_shot_white: [f64; 3],
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

/// Luminance relative en Rec. 2020.
pub const LUMA_REC2020: [f32; 3] = [0.2627, 0.678, 0.0593];

/// Paramètres figés du processus 1 : ce que « neutre » veut dire, et ce que
/// font les curseurs. Tous les réglages à zéro donnent exactement le rendu
/// neutre.
pub mod process1 {
    /// Décalage d'exposition de base : les boîtiers placent le gris moyen
    /// plus bas dans le RAW que dans leur JPEG. Mesuré sur les six formats de
    /// test-assets : à +1 IL, la luminance moyenne du rendu neutre égale celle
    /// du JPEG du boîtier (moyenne géométrique des rapports 1,00 ; de 0,88 à 1,20).
    pub const BASELINE_EV: f32 = 1.0;
    /// Début de l'épaule de la courbe : au-dessus, les hautes lumières sont
    /// comprimées en douceur vers le blanc au lieu d'être écrêtées.
    pub const SHOULDER: f32 = 0.6;
    /// Tonalité : courbe sur la luminance perçue `p = Y^(1/GAMMA)`, appliquée
    /// en rapport sur les trois canaux pour garder la teinte.
    pub const TONE_GAMMA: f32 = 2.2;
    /// Gris moyen (18 %) sur cette échelle : pivot du contraste.
    pub const TONE_MID: f32 = 0.458_659_6;
    /// Amplitude de chaque curseur à ±100, sur des formes qui laissent le noir
    /// et le blanc en place (luminosité, contraste, hautes lumières, ombres)
    /// ou les déplacent (blancs, noirs). Choisies pour que la courbe reste
    /// croissante curseur par curseur, même à fond.
    pub const BRIGHTNESS: f32 = 0.5;
    pub const CONTRAST: f32 = 0.4;
    pub const HIGHLIGHTS: f32 = 0.8;
    pub const SHADOWS: f32 = 0.8;
    pub const WHITES: f32 = 0.15;
    pub const BLACKS: f32 = 0.15;
}

/// Bits de [`RenderParams::flags`], communs au shader.
pub const FLAG_TONE: u32 = 1;
pub const FLAG_CLIP_HIGHLIGHTS: u32 = 2;
pub const FLAG_CLIP_SHADOWS: u32 = 4;

/// Couleurs des avertissements d'écrêtage peints sur l'image.
pub const CLIP_HIGHLIGHT_RGB: [u8; 3] = [255, 40, 40];
pub const CLIP_SHADOW_RGB: [u8; 3] = [40, 110, 255];

/// Histogramme : 256 cases pour le rouge, le vert, le bleu puis la luminance.
pub const HISTOGRAM_BINS: usize = 256 * 4;

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

/// Courbe de tonalité sur l'échelle perçue (0 = noir, 1 = blanc).
/// `t` : luminosité, contraste, hautes lumières, ombres, blancs, noirs, déjà
/// multipliés par leur amplitude.
#[inline]
pub fn tone_curve(p: f32, t: &[f32; 6]) -> f32 {
    let w = 1.0 - p;
    let out = p
        + t[0] * p * w
        + t[1] * 4.0 * (p - process1::TONE_MID) * p * w
        + t[2] * p * p * p * w
        + t[3] * p * w * w * w
        + t[4] * p * p * p * p
        + t[5] * w * w * w * w;
    out.max(0.0)
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

/// Coefficients d'une passe de réduction par moyenne de surface : chaque
/// pixel d'arrivée couvre `src / dst` pixels de départ, bords fractionnaires
/// compris.
fn area_weights(src: usize, dst: usize) -> Vec<(usize, Vec<f32>)> {
    let scale = src as f64 / dst as f64;
    (0..dst)
        .map(|o| {
            let (a, b) = (o as f64 * scale, (o + 1) as f64 * scale);
            let first = a.floor() as usize;
            let last = (b.ceil() as usize).min(src);
            let weights = (first..last)
                .map(|i| ((b.min(i as f64 + 1.0) - a.max(i as f64)) / scale) as f32)
                .collect();
            (first, weights)
        })
        .collect()
}

fn workers() -> usize {
    thread::available_parallelism().map_or(4, |n| n.get())
}

impl WorkingImage {
    /// Depuis le RAW développé par LibRaw, réduit pour que le côté long ne
    /// dépasse pas `max_side` (moyenne de surface, en linéaire, jamais agrandi).
    pub fn from_raw(raw: &DevelopedRaw, max_side: u32) -> Self {
        let (w, h) = (raw.width as usize, raw.height as usize);
        let long = w.max(h).max(1);
        let scale = (f64::from(max_side.max(1)) / long as f64).min(1.0);
        let ow = ((w as f64 * scale).round() as usize).clamp(1, w.max(1));
        let oh = ((h as f64 * scale).round() as usize).clamp(1, h.max(1));
        let n = workers();

        // Passe horizontale : h lignes de ow pixels.
        let cols = area_weights(w, ow);
        let mut tmp = vec![0.0f32; ow * h * 3];
        let rows_per = h.div_ceil(n).max(1);
        thread::scope(|s| {
            for (k, chunk) in tmp.chunks_mut(rows_per * ow * 3).enumerate() {
                let cols = &cols;
                s.spawn(move || {
                    for (r, line) in chunk.chunks_mut(ow * 3).enumerate() {
                        let src = &raw.xyz[(k * rows_per + r) * w * 3..];
                        for (ox, (first, weights)) in cols.iter().enumerate() {
                            let mut acc = [0.0f32; 3];
                            for (j, wt) in weights.iter().enumerate() {
                                let i = (first + j) * 3;
                                acc[0] += f32::from(src[i]) * wt;
                                acc[1] += f32::from(src[i + 1]) * wt;
                                acc[2] += f32::from(src[i + 2]) * wt;
                            }
                            line[ox * 3..ox * 3 + 3].copy_from_slice(&acc);
                        }
                    }
                });
            }
        });

        // Passe verticale, puis XYZ (0 à 65535) vers Rec. 2020 (0 à 1).
        let rows = area_weights(h, oh);
        let mut rgb = vec![0.0f32; ow * oh * 3];
        let out_per = oh.div_ceil(n).max(1);
        thread::scope(|s| {
            for (k, chunk) in rgb.chunks_mut(out_per * ow * 3).enumerate() {
                let (rows, tmp) = (&rows, &tmp);
                s.spawn(move || {
                    for (r, line) in chunk.chunks_mut(ow * 3).enumerate() {
                        let (first, weights) = &rows[k * out_per + r];
                        for ox in 0..ow {
                            let mut acc = [0.0f32; 3];
                            for (j, wt) in weights.iter().enumerate() {
                                let i = ((first + j) * ow + ox) * 3;
                                acc[0] += tmp[i] * wt;
                                acc[1] += tmp[i + 1] * wt;
                                acc[2] += tmp[i + 2] * wt;
                            }
                            let xyz = acc.map(|v| v / 65535.0);
                            line[ox * 3..ox * 3 + 3].copy_from_slice(&mul(&XYZ_TO_REC2020, xyz));
                        }
                    }
                });
            }
        });
        Self {
            width: ow as u32,
            height: oh as u32,
            rgb,
            as_shot_white: raw.as_shot_white,
        }
    }

    /// Couleur moyenne (XYZ) d'un carré de 5 × 5 pixels autour de (x, y),
    /// pour la pipette ; `None` hors de l'image.
    pub fn sample_xyz(&self, x: u32, y: u32) -> Option<[f64; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let (mut acc, mut n) = ([0.0f64; 3], 0.0);
        for yy in y.saturating_sub(2)..(y + 3).min(self.height) {
            for xx in x.saturating_sub(2)..(x + 3).min(self.width) {
                let i = ((yy * self.width + xx) * 3) as usize;
                for (c, v) in acc.iter_mut().enumerate() {
                    *v += f64::from(self.rgb[i + c]);
                }
                n += 1.0;
            }
        }
        let rec = acc.map(|v| v / n);
        Some(white::apply(&rec2020_to_xyz(), rec))
    }
}

fn to_f64(m: &[[f32; 3]; 3]) -> white::Matrix {
    m.map(|r| r.map(f64::from))
}

fn rec2020_to_xyz() -> white::Matrix {
    white::invert(&to_f64(&XYZ_TO_REC2020))
}

/// Avertissements d'écrêtage demandés par l'interface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Clipping {
    pub highlights: bool,
    pub shadows: bool,
}

/// Paramètres de rendu dérivés des réglages, communs au processeur et au GPU.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderParams {
    /// Exposition et balance des blancs, en Rec. 2020 linéaire.
    pub matrix: [[f32; 3]; 3],
    /// Luminosité, contraste, hautes lumières, ombres, blancs, noirs (amplitudes).
    pub tone: [f32; 6],
    /// Saturation et vibrance, de -1 à +1.
    pub saturation: f32,
    pub vibrance: f32,
    pub flags: u32,
}

impl RenderParams {
    pub fn from_settings(
        settings: &DevelopSettings,
        as_shot: [f64; 3],
        clipping: Clipping,
    ) -> Self {
        // Processus 1 seulement pour l'instant ; un processus 2 aurait ses propres constantes.
        use process1 as p;
        let l = &settings.light;
        let c = &settings.color;
        let gain = (l.exposure + f64::from(p::BASELINE_EV)).exp2();
        let xyz = white::correction_xyz(&c.white_balance, as_shot);
        let to_rec = to_f64(&XYZ_TO_REC2020);
        let m = white::mul(&to_rec, &white::mul(&xyz, &rec2020_to_xyz()));
        let matrix = m.map(|r| r.map(|v| (v * gain) as f32));
        let s = |v: f64, amp: f32| (v / 100.0) as f32 * amp;
        let tone = [
            s(l.brightness, p::BRIGHTNESS),
            s(l.contrast, p::CONTRAST),
            s(l.highlights, p::HIGHLIGHTS),
            s(l.shadows, p::SHADOWS),
            s(l.whites, p::WHITES),
            s(l.blacks, p::BLACKS),
        ];
        let mut flags = 0;
        if tone.iter().any(|v| *v != 0.0) {
            flags |= FLAG_TONE;
        }
        if clipping.highlights {
            flags |= FLAG_CLIP_HIGHLIGHTS;
        }
        if clipping.shadows {
            flags |= FLAG_CLIP_SHADOWS;
        }
        Self {
            matrix,
            tone,
            saturation: (c.saturation / 100.0) as f32,
            vibrance: (c.vibrance / 100.0) as f32,
            flags,
        }
    }
}

/// Rendu d'un pixel de travail en sRGB codé (0 à 1).
#[inline]
pub fn render_pixel(rgb: [f32; 3], p: &RenderParams) -> [f32; 3] {
    let lin = mul(&p.matrix, rgb);
    let mut v = lin.map(neutral_curve);
    if p.flags & FLAG_TONE != 0 {
        let y = LUMA_REC2020[0] * v[0] + LUMA_REC2020[1] * v[1] + LUMA_REC2020[2] * v[2];
        if y > 1e-6 {
            let q = y.powf(1.0 / process1::TONE_GAMMA);
            let ratio = tone_curve(q, &p.tone).powf(process1::TONE_GAMMA) / y;
            v = v.map(|c| c * ratio);
        }
    }
    if p.saturation != 0.0 || p.vibrance != 0.0 {
        let y = LUMA_REC2020[0] * v[0] + LUMA_REC2020[1] * v[1] + LUMA_REC2020[2] * v[2];
        let max = v[0].max(v[1]).max(v[2]);
        let min = v[0].min(v[1]).min(v[2]);
        let chroma = if max > 1e-6 {
            ((max - min) / max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dull = 1.0 - chroma;
        let factor = (1.0 + p.saturation) * (1.0 + p.vibrance * dull * dull);
        v = v.map(|c| y + (c - y) * factor);
    }
    mul(&REC2020_TO_SRGB, v).map(srgb_encode)
}

/// Pixel codé sur 8 bits, comme le shader (`pack4x8unorm`).
#[inline]
pub fn quantize(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Case de luminance d'un pixel 8 bits (coefficients Rec. 709 sur les valeurs
/// codées), en arithmétique entière : identique au bit près sur le processeur
/// et sur tous les processeurs graphiques.
#[inline]
pub fn luma_bin(q: [u8; 3]) -> usize {
    let l = 2126 * u32::from(q[0]) + 7152 * u32::from(q[1]) + 722 * u32::from(q[2]);
    (((l + 5000) / 10_000) as usize).min(255)
}

/// Rendu : pixels RGBA 8 bits (avertissements d'écrêtage peints si demandés)
/// et histogramme de l'image rendue, avant avertissements.
pub struct Rendered {
    pub rgba: Vec<u8>,
    pub histogram: Vec<u32>,
}

/// Couleur affichée d'un pixel rendu : un avertissement d'écrêtage si demandé.
#[inline]
fn shown(q: [u8; 3], flags: u32) -> [u8; 3] {
    if flags & FLAG_CLIP_HIGHLIGHTS != 0 && q.contains(&255) {
        CLIP_HIGHLIGHT_RGB
    } else if flags & FLAG_CLIP_SHADOWS != 0 && q.contains(&0) {
        CLIP_SHADOW_RGB
    } else {
        q
    }
}

/// Peint les avertissements d'écrêtage sur des pixels RGBA déjà rendus.
pub fn paint_clipping(rgba: &mut [u8], flags: u32) {
    if flags & (FLAG_CLIP_HIGHLIGHTS | FLAG_CLIP_SHADOWS) == 0 {
        return;
    }
    let chunk = (rgba.len() / 4).div_ceil(workers()).max(1) * 4;
    thread::scope(|s| {
        for part in rgba.chunks_mut(chunk) {
            s.spawn(move || {
                for px in part.as_chunks_mut::<4>().0 {
                    let q = [px[0], px[1], px[2]];
                    px[..3].copy_from_slice(&shown(q, flags));
                }
            });
        }
    });
}

/// Histogramme de pixels RGBA rendus (sans avertissements).
pub fn histogram_of(rgba: &[u8]) -> Vec<u32> {
    let mut hist = vec![0u32; HISTOGRAM_BINS];
    for px in rgba.as_chunks::<4>().0 {
        let q = [px[0], px[1], px[2]];
        hist[usize::from(q[0])] += 1;
        hist[256 + usize::from(q[1])] += 1;
        hist[512 + usize::from(q[2])] += 1;
        hist[768 + luma_bin(q)] += 1;
    }
    hist
}

/// Rendu complet, réparti sur les cœurs du processeur.
pub fn render(image: &WorkingImage, params: &RenderParams) -> Rendered {
    let pixels = (image.width * image.height) as usize;
    let mut out = vec![0u8; pixels * 4];
    let chunk = pixels.div_ceil(workers()).max(1);
    let partial: Vec<Vec<u32>> = thread::scope(|scope| {
        let handles: Vec<_> = out
            .chunks_mut(chunk * 4)
            .enumerate()
            .map(|(n, slice)| {
                let src = &image.rgb[n * chunk * 3..];
                scope.spawn(move || {
                    let mut hist = vec![0u32; HISTOGRAM_BINS];
                    for (i, px) in slice.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                        let v = render_pixel([src[i * 3], src[i * 3 + 1], src[i * 3 + 2]], params);
                        let q = v.map(quantize);
                        hist[usize::from(q[0])] += 1;
                        hist[256 + usize::from(q[1])] += 1;
                        hist[512 + usize::from(q[2])] += 1;
                        hist[768 + luma_bin(q)] += 1;
                        px[..3].copy_from_slice(&shown(q, params.flags));
                        px[3] = 255;
                    }
                    hist
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| vec![0; HISTOGRAM_BINS]))
            .collect()
    });
    let mut histogram = vec![0u32; HISTOGRAM_BINS];
    for part in partial {
        for (a, b) in histogram.iter_mut().zip(part) {
            *a += b;
        }
    }
    Rendered {
        rgba: out,
        histogram,
    }
}

/// Rendu RGBA sans avertissements, pour les tests et l'export.
pub fn render_rgba(image: &WorkingImage, settings: &DevelopSettings) -> Vec<u8> {
    let params = RenderParams::from_settings(settings, image.as_shot_white, Clipping::default());
    render(image, &params).rgba
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::settings::WhiteBalance;

    fn neutral() -> RenderParams {
        RenderParams::from_settings(&DevelopSettings::default(), D65_WHITE, Clipping::default())
    }

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
        let mut p = neutral();
        p.matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let grey = mul(&XYZ_TO_REC2020, [0.950_47 * 0.18, 0.18, 1.088_83 * 0.18]);
        let out = render_pixel(grey, &p);
        assert!(
            (out[0] - out[1]).abs() < 2e-3 && (out[1] - out[2]).abs() < 2e-3,
            "{out:?}"
        );
        // 18 % de gris : environ 118/255 en sRGB.
        assert!((out[1] * 255.0 - 118.0).abs() < 2.0, "{out:?}");
    }

    #[test]
    fn reglages_a_zero_identiques_au_rendu_neutre() {
        let p = neutral();
        assert_eq!(p.flags, 0);
        assert_eq!(p.tone, [0.0; 6]);
        // Balance du boîtier : seule l'exposition de base reste dans la matrice.
        let gain = process1::BASELINE_EV.exp2();
        for (r, row) in p.matrix.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let want = if r == c { gain } else { 0.0 };
                assert!((v - want).abs() < 1e-4, "{:?}", p.matrix);
            }
        }
    }

    #[test]
    fn chaque_curseur_garde_la_courbe_croissante_et_va_dans_son_sens() {
        for (k, name) in [
            "luminosité",
            "contraste",
            "hautes lumières",
            "ombres",
            "blancs",
            "noirs",
        ]
        .iter()
        .enumerate()
        {
            for amp in [-1.0f32, 1.0] {
                let mut t = [0.0; 6];
                t[k] = amp
                    * [
                        process1::BRIGHTNESS,
                        process1::CONTRAST,
                        process1::HIGHLIGHTS,
                        process1::SHADOWS,
                        process1::WHITES,
                        process1::BLACKS,
                    ][k];
                let mut prev = -1.0;
                for i in 0..=1000 {
                    let v = tone_curve(i as f32 / 1000.0, &t);
                    assert!(v >= prev - 1e-6, "{name} {amp} : décroît en {i}");
                    prev = v;
                }
            }
        }
        let t = |k: usize, v: f32| {
            let mut t = [0.0; 6];
            t[k] = v;
            t
        };
        // Luminosité : tons moyens seulement, noir et blanc fixes.
        assert!(tone_curve(0.5, &t(0, 0.5)) > 0.55);
        assert_eq!(tone_curve(0.0, &t(0, 0.5)), 0.0);
        assert!((tone_curve(1.0, &t(0, 0.5)) - 1.0).abs() < 1e-6);
        // Contraste : le gris moyen ne bouge pas, les ombres descendent.
        assert!((tone_curve(process1::TONE_MID, &t(1, 0.4)) - process1::TONE_MID).abs() < 1e-6);
        assert!(tone_curve(0.2, &t(1, 0.4)) < 0.2 && tone_curve(0.8, &t(1, 0.4)) > 0.8);
        // Hautes lumières agissent en haut, ombres en bas.
        assert!(tone_curve(0.85, &t(2, -0.8)) < 0.8 && tone_curve(0.15, &t(2, -0.8)) > 0.145);
        assert!(tone_curve(0.2, &t(3, 0.8)) > 0.25 && tone_curve(0.9, &t(3, 0.8)) < 0.902);
        // Blancs et noirs déplacent le blanc et le noir.
        assert!(tone_curve(1.0, &t(4, 0.15)) > 1.1);
        assert_eq!(tone_curve(0.0, &t(5, -0.15)), 0.0);
        assert!(tone_curve(0.0, &t(5, 0.15)) > 0.1);
    }

    #[test]
    fn saturation_et_vibrance() {
        let red = [0.4f32, 0.1, 0.1];
        let mut settings = DevelopSettings::default();
        let base = render_pixel(red, &neutral());
        settings.color.saturation = -100.0;
        let p = RenderParams::from_settings(&settings, D65_WHITE, Clipping::default());
        let grey = render_pixel(red, &p);
        assert!((grey[0] - grey[1]).abs() < 2e-3 && (grey[1] - grey[2]).abs() < 2e-3);
        settings.color.saturation = 0.0;
        settings.color.vibrance = 100.0;
        let p = RenderParams::from_settings(&settings, D65_WHITE, Clipping::default());
        let vivid = render_pixel(red, &p);
        assert!(vivid[0] - vivid[1] > base[0] - base[1]);
        // Une couleur déjà vive bouge moins qu'une couleur terne.
        let dull = [0.2f32, 0.17, 0.17];
        let d0 = render_pixel(dull, &neutral());
        let d1 = render_pixel(dull, &p);
        let pure = [0.4f32, 0.0, 0.0];
        let s0 = render_pixel(pure, &neutral());
        let s1 = render_pixel(pure, &p);
        assert!((d1[0] - d1[1]) / (d0[0] - d0[1]) > (s1[0] - s1[1]) / (s0[0] - s0[1]));
    }

    #[test]
    fn balance_plus_chaude_rechauffe_l_image() {
        let mut settings = DevelopSettings::default();
        settings.color.white_balance = WhiteBalance::Custom {
            temperature: 9000.0,
            tint: 0.0,
        };
        let p = RenderParams::from_settings(&settings, D65_WHITE, Clipping::default());
        let grey = mul(&XYZ_TO_REC2020, [0.950_47 * 0.1, 0.1, 1.088_83 * 0.1]);
        let out = render_pixel(grey, &p);
        assert!(out[0] > out[2] + 0.03, "{out:?}");
    }

    #[test]
    fn reduction_par_surface_garde_la_moyenne() {
        let (w, h) = (300u32, 200u32);
        let xyz: Vec<u16> = (0..w * h)
            .flat_map(|i| {
                let v = (i % 997) as u16 * 60;
                [v, v, v]
            })
            .collect();
        let raw = DevelopedRaw {
            width: w,
            height: h,
            xyz,
            as_shot_white: D65_WHITE,
        };
        let img = WorkingImage::from_raw(&raw, 128);
        assert_eq!((img.width, img.height), (128, 85));
        let mean_in: f64 = raw
            .xyz
            .iter()
            .step_by(3)
            .map(|v| f64::from(*v))
            .sum::<f64>()
            / f64::from(w * h)
            / 65535.0;
        let y = rec2020_to_xyz();
        let mean_out: f64 = img
            .rgb
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| white::apply(&y, [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])[1])
            .sum::<f64>()
            / f64::from(img.width * img.height);
        assert!(
            (mean_in - mean_out).abs() / mean_in < 0.01,
            "{mean_in} {mean_out}"
        );
        // Jamais agrandie.
        let same = WorkingImage::from_raw(&raw, 4096);
        assert_eq!((same.width, same.height), (w, h));
    }

    #[test]
    fn histogramme_recalcule_identique() {
        let image = WorkingImage {
            width: 64,
            height: 8,
            rgb: (0..64 * 8 * 3).map(|i| (i % 97) as f32 / 40.0).collect(),
            as_shot_white: D65_WHITE,
        };
        let r = render(&image, &neutral());
        assert_eq!(histogram_of(&r.rgba), r.histogram);
        let mut painted = r.rgba.clone();
        paint_clipping(&mut painted, FLAG_CLIP_HIGHLIGHTS | FLAG_CLIP_SHADOWS);
        let mut p = neutral();
        p.flags |= FLAG_CLIP_HIGHLIGHTS | FLAG_CLIP_SHADOWS;
        assert_eq!(painted, render(&image, &p).rgba);
    }

    #[test]
    fn histogramme_et_ecretage() {
        let image = WorkingImage {
            width: 2,
            height: 1,
            rgb: vec![10.0, 10.0, 10.0, 0.0, 0.0, 0.0],
            as_shot_white: D65_WHITE,
        };
        let mut p = neutral();
        p.flags |= FLAG_CLIP_HIGHLIGHTS | FLAG_CLIP_SHADOWS;
        let r = render(&image, &p);
        // Le blanc écrêté est peint en rouge, le noir en bleu ; l'histogramme
        // compte les valeurs réelles.
        assert_eq!(&r.rgba[0..3], &CLIP_HIGHLIGHT_RGB);
        assert_eq!(&r.rgba[4..7], &CLIP_SHADOW_RGB);
        assert_eq!(r.histogram[255], 1);
        assert_eq!(r.histogram[0], 1);
        assert_eq!(r.histogram[768 + 255], 1);
        assert_eq!(r.histogram.iter().sum::<u32>(), 8);
    }
}
