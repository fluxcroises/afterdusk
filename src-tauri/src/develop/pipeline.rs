//! Pipeline de développement, référence processeur (décision 0005) : chaque
//! étape a un jumeau WGSL (`gpu.rs`) qui doit donner le même résultat, à une
//! tolérance près vérifiée par les tests.
//!
//! Image de travail : linéaire, Rec. 2020, virgule flottante, 1.0 = blanc du
//! capteur, à la balance du boîtier. Rendu : exposition et balance des blancs
//! (une matrice), courbe neutre, tonalité, couleur, vignetage, sRGB, couleurs
//! par teinte (TSL), courbes de tonalité, grain, 8 bits, histogramme et
//! avertissements d'écrêtage.

use std::sync::Arc;
use std::thread;

use serde::Deserialize;

use super::curve;
use super::detail::{self, DetailParams, Radii};
use super::geometry;
use super::lens::LensCorrection;
use super::masks::{MaskSet, Planar};
use super::settings::{DevelopSettings, LocalAdjust, MASKS_MAX};
use super::spots;
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
    /// Côté de l'image de travail rapporté à celui de l'original en pleine
    /// définition (1 : pleine définition) : le grain, défini en pixels de
    /// l'original, garde ainsi la même taille apparente.
    pub scale: f32,
    /// Région d'une image plus grande (zoom 100 %) : sa place dans l'image
    /// entière, pour que vignetage et grain y soient les mêmes. `None` :
    /// l'image entière.
    pub crop: Option<Crop>,
    /// Image déjà développée (voir [`DevelopedRaw::rendered`]).
    pub rendered: bool,
}

/// Place d'une région dans l'image entière, en pixels de la région.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crop {
    pub left: u32,
    pub top: u32,
    pub full_width: u32,
    pub full_height: u32,
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
    /// Couleurs par teinte : décalage de teinte à ±100, en degrés ; part de
    /// la luminance modulée à ±100 (pondérée par la saturation).
    pub const HSL_HUE_DEGREES: f32 = 30.0;
    pub const HSL_LUMINANCE: f32 = 0.5;
    /// Vignetage : effet aux coins à ±100 ; rayon du point milieu (0 à 100)
    /// et largeur de la transition (progressivité 0 à 100), sur la demi-
    /// diagonale.
    pub const VIGNETTE_AMOUNT: f32 = 0.8;
    pub const VIGNETTE_MID_MIN: f32 = 0.3;
    pub const VIGNETTE_MID_RANGE: f32 = 0.6;
    pub const VIGNETTE_FEATHER_MIN: f32 = 0.05;
    pub const VIGNETTE_FEATHER_RANGE: f32 = 0.95;
    /// Grain : amplitude à 100 (en valeur codée) ; taille d'un grain en
    /// pixels de l'original, de 1 (taille 0) à 5 (taille 100).
    pub const GRAIN_AMPLITUDE: f32 = 0.18;
    pub const GRAIN_CELL_MIN: f32 = 1.0;
    pub const GRAIN_CELL_RANGE: f32 = 4.0;
    /// Retouches locales : décalage de température et de teinte à ±100, en IL
    /// sur les canaux rouge et bleu (température), vert (teinte).
    pub const LOCAL_TEMPERATURE: f32 = 0.5;
    pub const LOCAL_TINT: f32 = 0.35;
}

/// Centres des huit teintes (rouge, orange, jaune, vert, cyan, bleu, violet,
/// magenta), en degrés ; entre deux centres, les réglages s'interpolent.
pub const HUE_CENTERS: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];

/// Bits de [`RenderParams::flags`], communs au shader.
pub const FLAG_TONE: u32 = 1;
pub const FLAG_CLIP_HIGHLIGHTS: u32 = 2;
pub const FLAG_CLIP_SHADOWS: u32 = 4;
pub const FLAG_CURVE: u32 = 8;
pub const FLAG_HSL: u32 = 16;
pub const FLAG_VIGNETTE: u32 = 32;
pub const FLAG_GRAIN: u32 = 64;
pub const FLAG_DENOISE: u32 = 128;
pub const FLAG_SHARPEN: u32 = 256;
/// Source déjà développée : courbe neutre réduite à l'identité (bornée).
pub const FLAG_RENDERED: u32 = 512;
/// Retouches locales (décision 0014) : voir [`RenderParams::locals`].
pub const FLAG_MASKS: u32 = 1024;

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

pub(crate) fn workers() -> usize {
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
            rendered: raw.rendered,
            scale: ow.max(oh) as f32 / raw.full_long_side.max(1) as f32,
            crop: None,
        }
    }

    /// Région de l'original développé en pleine définition, sans réduction :
    /// un pixel de la région est un pixel de l'original. Bornée à l'image.
    pub fn crop_from_raw(raw: &DevelopedRaw, left: u32, top: u32, width: u32, height: u32) -> Self {
        let left = left.min(raw.width.saturating_sub(1));
        let top = top.min(raw.height.saturating_sub(1));
        let w = width.clamp(1, raw.width - left);
        let h = height.clamp(1, raw.height - top);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in top..top + h {
            let row = (y * raw.width + left) as usize * 3;
            for px in raw.xyz[row..row + w as usize * 3].as_chunks::<3>().0 {
                let xyz = px.map(|v| f32::from(v) / 65535.0);
                rgb.extend_from_slice(&mul(&XYZ_TO_REC2020, xyz));
            }
        }
        Self {
            width: w,
            height: h,
            rgb,
            as_shot_white: raw.as_shot_white,
            rendered: raw.rendered,
            scale: raw.width.max(raw.height) as f32 / raw.full_long_side.max(1) as f32,
            crop: Some(Crop {
                left,
                top,
                full_width: raw.width,
                full_height: raw.height,
            }),
        }
    }

    /// Région (left, top, width, height) de cette image (sa place dans
    /// l'image entière, voir [`Self::frame`]), bornée à l'image entière.
    pub fn sub(&self, left: u32, top: u32, width: u32, height: u32) -> Self {
        let (ox, oy, fw, fh) = self.frame();
        let l = left.clamp(ox, ox + self.width - 1);
        let t = top.clamp(oy, oy + self.height - 1);
        let w = width.clamp(1, ox + self.width - l);
        let h = height.clamp(1, oy + self.height - t);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in t - oy..t - oy + h {
            let row = ((y * self.width + (l - ox)) * 3) as usize;
            rgb.extend_from_slice(&self.rgb[row..row + (w * 3) as usize]);
        }
        Self {
            width: w,
            height: h,
            rgb,
            as_shot_white: self.as_shot_white,
            rendered: self.rendered,
            scale: self.scale,
            crop: Some(Crop {
                left: l,
                top: t,
                full_width: fw,
                full_height: fh,
            }),
        }
    }

    /// Région `rect` (left, top, width, height) de l'image *rendue* (après
    /// corrections d'objectif, de défauts et géométrie) en pleine définition,
    /// pour le zoom : agrandie de la portée des passes de voisinage, avec la
    /// partie à garder de son rendu (gauche, haut, largeur, hauteur). Seul le
    /// rectangle utile de l'original est converti.
    pub fn region_with_margin(
        raw: &DevelopedRaw,
        params: &RenderParams,
        settings: &DevelopSettings,
        lens: Option<&LensCorrection>,
        rect: (u32, u32, u32, u32),
    ) -> (Self, (u32, u32, u32, u32)) {
        let (left, top, width, height) = rect;
        let (geometry, spot_list) = (&settings.geometry, settings.spots.as_slice());
        let margin = if params.flags & (FLAG_DENOISE | FLAG_SHARPEN) != 0 {
            Radii::at(&params.detail, 1.0).margin()
        } else {
            0
        };
        let out = geometry::plan(geometry, f64::from(raw.width), f64::from(raw.height));
        let left = left.min(out.width.saturating_sub(1));
        let top = top.min(out.height.saturating_sub(1));
        let width = width.clamp(1, out.width - left);
        let height = height.clamp(1, out.height - top);
        let (l0, t0) = (left.saturating_sub(margin), top.saturating_sub(margin));
        let w0 = (width + (left - l0) + margin).min(out.width - l0);
        let h0 = (height + (top - t0) + margin).min(out.height - t0);
        // Original corrigé de l'objectif, lu par régions.
        let read = |l, t, w, h| Self::crop_from_raw(raw, l, t, w, h);
        let fetch = |l, t, w, h| match lens {
            Some(c) => c.region(raw.width, raw.height, (l, t, w, h), &read),
            None => read(l, t, w, h),
        };
        // Corrections de défauts sur la partie utile, les sources lues dans
        // l'image entière.
        let heal = |mut img: Self| {
            if !spot_list.is_empty() {
                spots::heal(&mut img, spot_list, &fetch);
            }
            img
        };
        let region = if geometry.is_identity() {
            heal(fetch(l0, t0, w0, h0))
        } else {
            let (x0, y0, x1, y1) =
                geometry::source_bounds(geometry, raw.width, raw.height, l0, t0, w0, h0);
            let source = heal(fetch(x0, y0, x1 - x0, y1 - y0));
            geometry::transform_region(&source, geometry, l0, t0, w0, h0)
        };
        (region, (left - l0, top - t0, width, height))
    }

    /// Copie réduite (moyenne de surface) dont le côté long ne dépasse pas
    /// `max_side`, pour les vignettes des presets ; jamais agrandie.
    pub fn downscaled(&self, max_side: u32) -> Self {
        let (w, h) = (self.width as usize, self.height as usize);
        let k = (f64::from(max_side.max(1)) / w.max(h).max(1) as f64).min(1.0);
        let ow = ((w as f64 * k).round() as usize).clamp(1, w.max(1));
        let oh = ((h as f64 * k).round() as usize).clamp(1, h.max(1));
        self.resized(ow as u32, oh as u32)
    }

    /// Copie à la taille exacte `ow` × `oh` (moyenne de surface), image
    /// entière ; le grain garde sa taille apparente.
    pub fn resized(&self, ow: u32, oh: u32) -> Self {
        let (w, h) = (self.width as usize, self.height as usize);
        let (ow, oh) = (ow.max(1) as usize, oh.max(1) as usize);
        let (cols, rows) = (area_weights(w, ow), area_weights(h, oh));
        let mut tmp = vec![0.0f32; ow * h * 3];
        for y in 0..h {
            for (ox, (first, weights)) in cols.iter().enumerate() {
                for (j, wt) in weights.iter().enumerate() {
                    let i = (y * w + first + j) * 3;
                    for c in 0..3 {
                        tmp[(y * ow + ox) * 3 + c] += self.rgb[i + c] * wt;
                    }
                }
            }
        }
        let mut rgb = vec![0.0f32; ow * oh * 3];
        for (oy, (first, weights)) in rows.iter().enumerate() {
            for (j, wt) in weights.iter().enumerate() {
                for ox in 0..ow {
                    let i = ((first + j) * ow + ox) * 3;
                    for c in 0..3 {
                        rgb[(oy * ow + ox) * 3 + c] += tmp[i + c] * wt;
                    }
                }
            }
        }
        Self {
            width: ow as u32,
            height: oh as u32,
            rgb,
            as_shot_white: self.as_shot_white,
            rendered: self.rendered,
            scale: self.scale * ow.max(oh) as f32 / w.max(h).max(1) as f32,
            crop: None,
        }
    }

    /// Origine de l'image dans l'image entière, et taille de celle-ci.
    pub fn frame(&self) -> (u32, u32, u32, u32) {
        self.crop.map_or((0, 0, self.width, self.height), |c| {
            (c.left, c.top, c.full_width, c.full_height)
        })
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

/// Avertissements d'écrêtage demandés par l'interface, et visualisation des
/// poussières (correcteur de défauts).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Clipping {
    pub highlights: bool,
    pub shadows: bool,
    pub dust: bool,
}

/// Paramètres de rendu dérivés des réglages, communs au processeur et au GPU.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderParams {
    /// Exposition et balance des blancs, en Rec. 2020 linéaire.
    pub matrix: [[f32; 3]; 3],
    /// Luminosité, contraste, hautes lumières, ombres, blancs, noirs (amplitudes).
    pub tone: [f32; 6],
    /// Saturation et vibrance, de -1 à +1.
    pub saturation: f32,
    pub vibrance: f32,
    pub flags: u32,
    /// Couleurs par teinte, pour les huit centres : décalage de teinte (en
    /// degrés), saturation et luminance (de -1 à +1).
    pub hsl: [[f32; 8]; 3],
    /// Vignetage : effet aux coins, rayon du point milieu, largeur.
    pub vignette: [f32; 3],
    /// Grain : amplitude, taille d'un grain (pixels de l'original), irrégularité (0 à 1).
    pub grain: [f32; 3],
    /// Tables des courbes (RVB, rouge, vert, bleu), vide sans courbe.
    pub curve: Vec<f32>,
    /// Réduction du bruit et netteté (passes de voisinage).
    pub detail: DetailParams,
    /// Retouches locales : masques et réglages de chacun, ou rien.
    pub locals: Option<Arc<Locals>>,
}

/// Réglages d'un masque, prêts pour le rendu (à plein effet).
#[derive(Debug, Clone, PartialEq)]
pub struct LocalParams {
    /// Gains en IL sur les canaux rouge, vert et bleu (Rec. 2020 linéaire) :
    /// exposition, température, teinte.
    pub log_gain: [f32; 3],
    /// Amplitudes de la courbe de tonalité (même forme que [`RenderParams::tone`]).
    pub tone: [f32; 6],
    pub has_tone: bool,
    /// Saturation, de -1 à +1.
    pub saturation: f32,
}

impl LocalParams {
    fn from_adjust(a: &LocalAdjust) -> Self {
        use process1 as p;
        let pct = |v: f64| (v / 100.0) as f32;
        let ev = a.exposure as f32;
        let warm = pct(a.temperature) * p::LOCAL_TEMPERATURE;
        let magenta = pct(a.tint) * p::LOCAL_TINT;
        let tone = [
            0.0,
            pct(a.contrast) * p::CONTRAST,
            pct(a.highlights) * p::HIGHLIGHTS,
            pct(a.shadows) * p::SHADOWS,
            0.0,
            0.0,
        ];
        Self {
            log_gain: [ev + warm, ev - magenta, ev - warm],
            has_tone: tone.iter().any(|v| *v != 0.0),
            tone,
            saturation: pct(a.saturation),
        }
    }
}

/// Masques actifs d'une photo et leurs réglages, dans le même ordre.
#[derive(Debug, Clone, PartialEq)]
pub struct Locals {
    pub masks: MaskSet,
    /// Les mêmes formes en pixels de la sortie (voir [`MaskSet::planar`]) :
    /// une forme affine par pixel au lieu de la géométrie entière, comme sur
    /// la carte graphique.
    pub planar: Vec<Planar>,
    pub params: Vec<LocalParams>,
}

impl Locals {
    /// Poids de chaque masque au pixel `at` (sortie entière), dans `out`.
    fn weights(&self, at: &At, out: &mut [f32; MASKS_MAX]) -> usize {
        let (x, y) = (f64::from(at.x) + 0.5, f64::from(at.y) + 0.5);
        let n = self.planar.len().min(MASKS_MAX);
        for (w, shape) in out.iter_mut().zip(&self.planar) {
            *w = shape.weight(x, y) as f32;
        }
        n
    }
}

impl RenderParams {
    /// Source déjà développée (JPEG, TIFF, PNG, HEIF) : ni décalage
    /// d'exposition de base ni épaule, pour que les réglages à zéro redonnent
    /// l'image telle quelle.
    #[must_use]
    pub fn rendered(mut self, rendered: bool) -> Self {
        if rendered {
            let undo = (-process1::BASELINE_EV).exp2();
            self.matrix = self.matrix.map(|r| r.map(|v| v * undo));
            self.flags |= FLAG_RENDERED;
        }
        self
    }

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
        let h = &settings.hsl;
        let pct = |v: f64| (v / 100.0) as f32;
        let hsl = [
            h.hue.values().map(|v| pct(v) * p::HSL_HUE_DEGREES),
            h.saturation.values().map(pct),
            h.luminance.values().map(|v| pct(v) * p::HSL_LUMINANCE),
        ];
        if !h.is_default() {
            flags |= FLAG_HSL;
        }
        let v = &settings.effects.vignette;
        let vignette = [
            pct(v.amount) * p::VIGNETTE_AMOUNT,
            p::VIGNETTE_MID_MIN + pct(v.midpoint) * p::VIGNETTE_MID_RANGE,
            p::VIGNETTE_FEATHER_MIN + pct(v.feather) * p::VIGNETTE_FEATHER_RANGE,
        ];
        if v.amount != 0.0 {
            flags |= FLAG_VIGNETTE;
        }
        let g = &settings.effects.grain;
        let grain = [
            pct(g.amount) * p::GRAIN_AMPLITUDE,
            p::GRAIN_CELL_MIN + pct(g.size) * p::GRAIN_CELL_RANGE,
            pct(g.roughness),
        ];
        if g.amount != 0.0 {
            flags |= FLAG_GRAIN;
        }
        let curve = curve::tables(&settings.curve).unwrap_or_default();
        if !curve.is_empty() {
            flags |= FLAG_CURVE;
        }
        let detail = DetailParams::from_settings(&settings.detail);
        if detail.denoises() {
            flags |= FLAG_DENOISE;
        }
        if detail.sharpens() {
            flags |= FLAG_SHARPEN;
        }
        Self {
            matrix,
            tone,
            saturation: (c.saturation / 100.0) as f32,
            vibrance: (c.vibrance / 100.0) as f32,
            flags,
            hsl,
            vignette,
            grain,
            curve,
            detail,
            locals: None,
        }
    }

    /// Ajoute les retouches locales des réglages, pour une image de travail
    /// de `sw` × `sh` pixels avant géométrie (celle que la géométrie
    /// transforme pour donner l'image rendue). Sortie redimensionnée après la
    /// géométrie : la source à l'échelle de la sortie, en pixels fractionnaires.
    #[must_use]
    pub fn with_masks(mut self, settings: &DevelopSettings, sw: f64, sh: f64) -> Self {
        let masks = MaskSet::sized(&settings.masks, &settings.geometry, sw, sh);
        if masks.is_empty() {
            return self;
        }
        let params = masks
            .fields
            .iter()
            .map(|f| LocalParams::from_adjust(&f.adjust))
            .collect();
        let planar = masks.planar();
        self.locals = Some(Arc::new(Locals {
            masks,
            planar,
            params,
        }));
        self.flags |= FLAG_MASKS;
        self
    }
}

/// Position d'un pixel dans l'image rendue, pour le vignetage et le grain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct At {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Échelle de l'image de travail (voir [`WorkingImage::scale`]).
    pub scale: f32,
}

impl At {
    /// Centre d'une image d'un pixel : ni vignetage, ni grain particulier.
    pub const CENTER: Self = Self {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
        scale: 1.0,
    };
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Facteur de vignetage d'un pixel : 1 au centre, `1 + effet` aux coins.
#[inline]
pub fn vignette_factor(v: &[f32; 3], at: &At) -> f32 {
    let u = (at.x as f32 + 0.5) / at.width as f32 * 2.0 - 1.0;
    let w = (at.y as f32 + 0.5) / at.height as f32 * 2.0 - 1.0;
    let r = ((u * u + w * w) * 0.5).sqrt();
    let m = smoothstep(v[1] - v[2] * 0.5, v[1] + v[2] * 0.5, r);
    1.0 + v[0] * m
}

/// Couleurs par teinte, sur des valeurs codées (0 à 1) : teinte, saturation et
/// luminance de chaque couleur, interpolées entre les deux centres voisins.
/// Les gris, sans teinte, ne bougent pas.
#[inline]
pub fn apply_hsl(c: [f32; 3], hsl: &[[f32; 8]; 3]) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let d = mx - mn;
    if d <= 1e-6 || mx <= 0.0 {
        return c;
    }
    let s = d / mx;
    let mut h = if mx == c[0] {
        60.0 * (c[1] - c[2]) / d
    } else if mx == c[1] {
        60.0 * ((c[2] - c[0]) / d + 2.0)
    } else {
        60.0 * ((c[0] - c[1]) / d + 4.0)
    };
    if h < 0.0 {
        h += 360.0;
    }
    let mut seg = 7;
    for i in 0..7 {
        if h < HUE_CENTERS[i + 1] {
            seg = i;
            break;
        }
    }
    let c0 = HUE_CENTERS[seg];
    let c1 = if seg == 7 {
        360.0
    } else {
        HUE_CENTERS[seg + 1]
    };
    // Lissée : un plateau autour de chaque centre, sans saut entre deux.
    let t = smoothstep(0.0, 1.0, (h - c0) / (c1 - c0));
    let next = (seg + 1) % 8;
    let adj = |k: usize| hsl[k][seg] * (1.0 - t) + hsl[k][next] * t;
    let mut h2 = h + adj(0);
    if h2 < 0.0 {
        h2 += 360.0;
    }
    if h2 >= 360.0 {
        h2 -= 360.0;
    }
    let s2 = (s * (1.0 + adj(1))).clamp(0.0, 1.0);
    let v2 = (mx * (1.0 + adj(2) * s)).clamp(0.0, 1.0);
    // TSV vers RVB.
    let chroma = v2 * s2;
    let hh = h2 / 60.0;
    let x = chroma * (1.0 - ((hh % 2.0) - 1.0).abs());
    let m = v2 - chroma;
    let (r, g, b) = match hh as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    [r + m, g + m, b + m]
}

/// Courbes : l'ensemble RVB, puis la courbe de chaque canal.
#[inline]
pub fn apply_curve(c: [f32; 3], lut: &[f32]) -> [f32; 3] {
    let n = curve::LUT_SIZE;
    let rgb = &lut[..n];
    std::array::from_fn(|k| curve::sample(&lut[(k + 1) * n..(k + 2) * n], curve::sample(rgb, c[k])))
}

/// Hachage entier d'un nœud de la grille du grain (identique sur le GPU).
#[inline]
pub fn grain_hash(ix: i32, iy: i32, seed: u32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(0x8da6_b343)
        ^ (iy as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / 16_777_216.0
}

/// Bruit de valeur lissé, de 0 à 1.
#[inline]
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let (tx, ty) = (x - fx, y - fy);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = grain_hash(ix, iy, seed);
    let b = grain_hash(ix + 1, iy, seed);
    let c = grain_hash(ix, iy + 1, seed);
    let d = grain_hash(ix + 1, iy + 1, seed);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    top + (bottom - top) * sy
}

/// Grain monochrome, plus visible dans les tons moyens ; placé en pixels de
/// l'original, il garde sa taille apparente quelle que soit l'échelle.
#[inline]
pub fn apply_grain(c: [f32; 3], g: &[f32; 3], at: &At) -> [f32; 3] {
    let cell = g[1];
    let gx = (at.x as f32 + 0.5) / at.scale / cell;
    let gy = (at.y as f32 + 0.5) / at.scale / cell;
    let fine = value_noise(gx * 2.03, gy * 2.03, 1);
    let n = value_noise(gx, gy, 0) * (1.0 - g[2]) + fine * g[2];
    // Un pixel d'aperçu couvre plusieurs grains : leur moyenne varie moins.
    let attenuation = (cell * at.scale).min(1.0);
    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let weight = 0.25 + 3.0 * l * (1.0 - l);
    let delta = (n - 0.5) * g[0] * attenuation * weight;
    c.map(|v| v + delta)
}

/// Rendu d'un pixel de travail en sRGB codé (0 à 1), au centre d'une image
/// d'un pixel (ni vignetage, ni grain particulier).
#[inline]
pub fn render_pixel(rgb: [f32; 3], p: &RenderParams) -> [f32; 3] {
    render_pixel_at(rgb, p, &At::CENTER)
}

/// Rendu d'un pixel de travail en sRGB codé (0 à 1), à sa place dans l'image.
#[inline]
pub fn render_pixel_at(rgb: [f32; 3], p: &RenderParams, at: &At) -> [f32; 3] {
    let out = display_of(mul(&p.matrix, rgb), p, at);
    if p.flags & FLAG_GRAIN != 0 {
        apply_grain(out, &p.grain, at)
    } else {
        out
    }
}

/// De l'image linéaire (exposition et balance appliquées) aux valeurs codées,
/// avant netteté et grain : courbe neutre, tonalité, couleur, vignetage,
/// sRGB, TSL, courbes.
#[inline]
pub fn display_of(lin: [f32; 3], p: &RenderParams, at: &At) -> [f32; 3] {
    // Retouches locales : poids de chaque masque en ce pixel ; leurs réglages
    // s'additionnent selon les poids (gains en IL, amplitudes de la courbe de
    // tonalité, saturation), d'un seul passage.
    let mut log_gain = [0.0f32; 3];
    let mut local_tone = [0.0f32; 6];
    let mut local_saturation = 0.0f32;
    if let Some(l) = p.locals.as_deref().filter(|_| p.flags & FLAG_MASKS != 0) {
        let mut weights = [0.0f32; MASKS_MAX];
        let n = l.weights(at, &mut weights);
        for (lp, &w) in l.params[..n].iter().zip(&weights) {
            if w <= 0.0 {
                continue;
            }
            for (g, v) in log_gain.iter_mut().zip(lp.log_gain) {
                *g += w * v;
            }
            if lp.has_tone {
                for (t, v) in local_tone.iter_mut().zip(lp.tone) {
                    *t += w * v;
                }
            }
            local_saturation += w * lp.saturation;
        }
    }
    let mut lin = lin;
    if log_gain != [0.0; 3] {
        lin = std::array::from_fn(|c| lin[c] * log_gain[c].exp2());
    }
    let mut v = if p.flags & FLAG_RENDERED != 0 {
        lin.map(|x| x.clamp(0.0, 1.0))
    } else {
        lin.map(neutral_curve)
    };
    let local_tone = (local_tone != [0.0; 6]).then_some(local_tone);
    if p.flags & FLAG_TONE != 0 || local_tone.is_some() {
        let y = LUMA_REC2020[0] * v[0] + LUMA_REC2020[1] * v[1] + LUMA_REC2020[2] * v[2];
        if y > 1e-6 {
            let q = y.powf(1.0 / process1::TONE_GAMMA);
            let mut t = if p.flags & FLAG_TONE != 0 {
                tone_curve(q, &p.tone)
            } else {
                q
            };
            // Courbe des masques, de leurs amplitudes pondérées, après celle
            // de la photo.
            if let Some(local) = &local_tone {
                t = tone_curve(t, local);
            }
            let ratio = t.max(0.0).powf(process1::TONE_GAMMA) / y;
            v = v.map(|c| c * ratio);
        }
    }
    if p.saturation != 0.0 || p.vibrance != 0.0 || local_saturation != 0.0 {
        let y = LUMA_REC2020[0] * v[0] + LUMA_REC2020[1] * v[1] + LUMA_REC2020[2] * v[2];
        let max = v[0].max(v[1]).max(v[2]);
        let min = v[0].min(v[1]).min(v[2]);
        let chroma = if max > 1e-6 {
            ((max - min) / max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dull = 1.0 - chroma;
        let factor = (1.0 + p.saturation)
            * (1.0 + p.vibrance * dull * dull)
            * (1.0 + local_saturation).max(0.0);
        v = v.map(|c| y + (c - y) * factor);
    }
    if p.flags & FLAG_VIGNETTE != 0 {
        let f = vignette_factor(&p.vignette, at);
        v = v.map(|c| c * f);
    }
    let mut out = mul(&REC2020_TO_SRGB, v).map(srgb_encode);
    if p.flags & FLAG_HSL != 0 {
        out = apply_hsl(out, &p.hsl);
    }
    if p.flags & FLAG_CURVE != 0 {
        out = apply_curve(out, &p.curve);
    }
    out
}

/// Rendu par passes, quand une étape regarde les voisins : image linéaire,
/// réduction du bruit, valeurs codées, netteté. Renvoie les valeurs codées
/// (trois par pixel), avant grain.
fn staged_display(image: &WorkingImage, p: &RenderParams) -> Vec<f32> {
    let (w, h) = (image.width as usize, image.height as usize);
    let (ox, oy, fw, fh) = image.frame();
    let radii = Radii::at(&p.detail, image.scale);
    let mut lin: Vec<f32> = image
        .rgb
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|c| mul(&p.matrix, *c))
        .collect();
    if p.flags & FLAG_DENOISE != 0 {
        detail::denoise(&mut lin, w, h, &p.detail, &radii);
    }
    let mut enc = vec![0.0f32; w * h * 3];
    let rows = h.div_ceil(workers()).max(1);
    thread::scope(|s| {
        for (k, chunk) in enc.chunks_mut(rows * w * 3).enumerate() {
            let lin = &lin;
            s.spawn(move || {
                for (i, out) in chunk.as_chunks_mut::<3>().0.iter_mut().enumerate() {
                    let index = k * rows * w + i;
                    let at = At {
                        x: ox + (index % w) as u32,
                        y: oy + (index / w) as u32,
                        width: fw,
                        height: fh,
                        scale: image.scale,
                    };
                    let px = [lin[index * 3], lin[index * 3 + 1], lin[index * 3 + 2]];
                    *out = display_of(px, p, &at);
                }
            });
        }
    });
    if p.flags & FLAG_SHARPEN != 0 {
        detail::sharpen(&mut enc, w, h, &p.detail, &radii);
    }
    enc
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
    let (ox, oy, fw, fh) = image.frame();
    let staged =
        (params.flags & (FLAG_DENOISE | FLAG_SHARPEN) != 0).then(|| staged_display(image, params));
    let staged = staged.as_deref();
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
                        let index = n * chunk + i;
                        let at = At {
                            x: ox + (index % image.width as usize) as u32,
                            y: oy + (index / image.width as usize) as u32,
                            width: fw,
                            height: fh,
                            scale: image.scale,
                        };
                        let v = match staged {
                            Some(enc) => {
                                let e = [enc[index * 3], enc[index * 3 + 1], enc[index * 3 + 2]];
                                if params.flags & FLAG_GRAIN != 0 {
                                    apply_grain(e, &params.grain, &at)
                                } else {
                                    e
                                }
                            }
                            None => render_pixel_at(
                                [src[i * 3], src[i * 3 + 1], src[i * 3 + 2]],
                                params,
                                &at,
                            ),
                        };
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

/// Rendu en valeurs sRGB codées (0 à 1), en virgule flottante et sans
/// quantification : pour l'export (16 bits, autres espaces colorimétriques).
/// Mêmes calculs que [`render`], quantifiés près.
pub fn render_encoded(image: &WorkingImage, params: &RenderParams) -> Vec<f32> {
    let (ox, oy, fw, fh) = image.frame();
    let w = image.width as usize;
    let mut enc = if params.flags & (FLAG_DENOISE | FLAG_SHARPEN) != 0 {
        staged_display(image, params)
    } else {
        image.rgb.clone()
    };
    let staged = params.flags & (FLAG_DENOISE | FLAG_SHARPEN) != 0;
    let rows = (image.height as usize).div_ceil(workers()).max(1);
    thread::scope(|s| {
        for (k, chunk) in enc.chunks_mut(rows * w * 3).enumerate() {
            s.spawn(move || {
                for (i, px) in chunk.as_chunks_mut::<3>().0.iter_mut().enumerate() {
                    let index = k * rows * w + i;
                    let at = At {
                        x: ox + (index % w) as u32,
                        y: oy + (index / w) as u32,
                        width: fw,
                        height: fh,
                        scale: image.scale,
                    };
                    *px = if staged {
                        if params.flags & FLAG_GRAIN != 0 {
                            apply_grain(*px, &params.grain, &at)
                        } else {
                            *px
                        }
                    } else {
                        render_pixel_at(*px, params, &at)
                    };
                }
            });
        }
    });
    enc
}

/// Rendu RGBA sans avertissements, pour les tests et l'export.
pub fn render_rgba(image: &WorkingImage, settings: &DevelopSettings) -> Vec<u8> {
    let params = RenderParams::from_settings(settings, image.as_shot_white, Clipping::default());
    render(image, &params).rgba
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::settings::{CropRect, LocalAdjust, Mask, MaskShape, WhiteBalance};

    fn neutral() -> RenderParams {
        RenderParams::from_settings(&DevelopSettings::default(), D65_WHITE, Clipping::default())
    }

    #[test]
    fn rendu_en_flottants_identique_au_rendu_8_bits() {
        let (w, h) = (40u32, 30u32);
        let xyz: Vec<u16> = (0..w * h)
            .flat_map(|i| {
                let v = ((i * 37) % 60000) as u16;
                [v, v / 2, v / 3]
            })
            .collect();
        let raw = DevelopedRaw {
            width: w,
            height: h,
            xyz,
            as_shot_white: D65_WHITE,
            full_long_side: w,
            rendered: false,
        };
        let image = WorkingImage::from_raw(&raw, 40);
        for detail in [false, true] {
            let mut s = DevelopSettings::default();
            s.light.contrast = 30.0;
            s.effects.grain.amount = 40.0;
            s.effects.vignette.amount = -30.0;
            if detail {
                s.detail.sharpening.amount = 80.0;
                s.detail.noise.luminance = 40.0;
            }
            let p = RenderParams::from_settings(&s, D65_WHITE, Clipping::default());
            let rgba = render(&image, &p).rgba;
            let enc = render_encoded(&image, &p);
            for (q, e) in rgba.chunks(4).zip(enc.chunks(3)) {
                for c in 0..3 {
                    assert_eq!(q[c], quantize(e[c]));
                }
            }
        }
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
            full_long_side: w,
            rendered: false,
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

    fn with(f: impl FnOnce(&mut DevelopSettings)) -> RenderParams {
        let mut s = DevelopSettings::default();
        f(&mut s);
        RenderParams::from_settings(&s, D65_WHITE, Clipping::default())
    }

    #[test]
    fn tsl_n_agit_que_sur_sa_teinte() {
        let red = [0.8f32, 0.2, 0.15];
        let blue = [0.15f32, 0.25, 0.8];
        let grey = [0.5f32, 0.5, 0.5];
        let p = with(|s| s.hsl.saturation.red = -100.0);
        let r = apply_hsl(red, &p.hsl);
        assert!(
            (r[0] - r[1]).abs() < 0.05 && (r[1] - r[2]).abs() < 0.05,
            "{r:?}"
        );
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
        assert!(close(apply_hsl(blue, &p.hsl), blue));
        assert_eq!(apply_hsl(grey, &p.hsl), grey);
        // Teinte : le rouge glisse vers l'orange ; luminance : le bleu s'assombrit.
        let p = with(|s| s.hsl.hue.red = 100.0);
        let r = apply_hsl(red, &p.hsl);
        assert!(r[1] > red[1] + 0.1, "{r:?}");
        let p = with(|s| s.hsl.luminance.blue = -100.0);
        let b = apply_hsl(blue, &p.hsl);
        assert!(b[2] < blue[2] - 0.1 && b.iter().all(|v| *v >= 0.0), "{b:?}");
        // Aller-retour TSV sans réglage : identique.
        let z = with(|_| {});
        let back = apply_hsl([0.3, 0.6, 0.2], &z.hsl);
        assert!(
            back.iter()
                .zip([0.3, 0.6, 0.2])
                .all(|(a, b)| (a - b).abs() < 1e-6)
        );
    }

    #[test]
    fn courbe_appliquee_au_rendu() {
        let p = with(|s| s.curve.rgb = vec![[0.0, 40.0], [255.0, 255.0]]);
        assert_ne!(p.flags & FLAG_CURVE, 0);
        let black = render_pixel([0.0, 0.0, 0.0], &p);
        assert!((black[0] - 40.0 / 255.0).abs() < 2e-3, "{black:?}");
        // Courbe du seul canal rouge : le vert et le bleu ne bougent pas.
        let p = with(|s| s.curve.red = vec![[0.0, 0.0], [128.0, 200.0], [255.0, 255.0]]);
        let base = render_pixel([0.1, 0.1, 0.1], &neutral());
        let out = render_pixel([0.1, 0.1, 0.1], &p);
        assert!(out[0] > base[0] + 0.05 && (out[1] - base[1]).abs() < 1e-6);
    }

    #[test]
    fn vignetage_aux_coins_seulement() {
        let p = with(|s| s.effects.vignette.amount = -100.0);
        let at = |x, y| At {
            x,
            y,
            width: 300,
            height: 200,
            scale: 1.0,
        };
        assert!((vignette_factor(&p.vignette, &at(150, 100)) - 1.0).abs() < 1e-6);
        let corner = vignette_factor(&p.vignette, &at(0, 0));
        assert!(corner < 0.3, "{corner}");
        let p = with(|s| s.effects.vignette.amount = 60.0);
        assert!(vignette_factor(&p.vignette, &at(299, 199)) > 1.3);
    }

    #[test]
    fn grain_deterministe_centre_et_a_l_echelle() {
        let p = with(|s| s.effects.grain.amount = 100.0);
        let px = |x, y, scale| At {
            x,
            y,
            width: 4000,
            height: 3000,
            scale,
        };
        let grey = [0.5f32, 0.5, 0.5];
        let a = apply_grain(grey, &p.grain, &px(10, 20, 1.0));
        assert_eq!(a, apply_grain(grey, &p.grain, &px(10, 20, 1.0)));
        // Moyenne nulle, écart sensible en pleine définition, atténué en aperçu.
        let spread = |scale: f32| {
            let v: Vec<f32> = (0..2000)
                .map(|i| apply_grain(grey, &p.grain, &px(i % 97 * 7, i / 97 * 5, scale))[0] - 0.5)
                .collect();
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            let var = v.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / v.len() as f32;
            (mean, var.sqrt())
        };
        let (mean, full) = spread(1.0);
        let (_, preview) = spread(0.2);
        assert!(mean.abs() < 0.01 && full > 0.01, "{mean} {full}");
        assert!(preview < full, "{preview} {full}");
        // Sans grain, rien ne bouge.
        assert_eq!(with(|_| {}).flags & FLAG_GRAIN, 0);
    }

    #[test]
    fn region_identique_a_l_image_entiere() {
        // Une image de 40 × 30 développée « en pleine définition ».
        let (w, h) = (40u32, 30u32);
        let xyz: Vec<u16> = (0..w * h)
            .flat_map(|i| {
                let v = ((i * 37) % 60000) as u16;
                [v, v / 2, v / 3]
            })
            .collect();
        let raw = DevelopedRaw {
            width: w,
            height: h,
            xyz,
            as_shot_white: D65_WHITE,
            full_long_side: w,
            rendered: false,
        };
        let whole = WorkingImage::from_raw(&raw, 40);
        let part = WorkingImage::crop_from_raw(&raw, 10, 5, 12, 8);
        assert_eq!((part.width, part.height, part.scale), (12, 8, 1.0));
        let mut s = DevelopSettings::default();
        s.effects.vignette.amount = -80.0;
        s.effects.grain.amount = 100.0;
        let p = RenderParams::from_settings(&s, D65_WHITE, Clipping::default());
        let a = render(&whole, &p).rgba;
        let b = render(&part, &p).rgba;
        // Chaque pixel de la région est celui de l'image entière (vignetage et
        // grain compris).
        for y in 0..8 {
            for x in 0..12 {
                let i = ((y * 12 + x) * 4) as usize;
                let j = (((y + 5) * 40 + x + 10) * 4) as usize;
                assert_eq!(b[i..i + 3], a[j..j + 3], "({x}, {y})");
            }
        }
        // Région débordante : ramenée à l'image.
        let edge = WorkingImage::crop_from_raw(&raw, 35, 28, 20, 20);
        assert_eq!((edge.width, edge.height), (5, 2));
    }

    #[test]
    fn rendu_par_passes_identique_au_rendu_direct() {
        // Netteté sur une image unie : le rendu par passes doit donner les
        // mêmes valeurs que le rendu direct (même chaîne, découpée).
        let image = WorkingImage {
            width: 16,
            height: 12,
            rgb: vec![0.12; 16 * 12 * 3],
            as_shot_white: D65_WHITE,
            scale: 1.0,
            crop: None,
            rendered: false,
        };
        let mut s = DevelopSettings::default();
        s.light.contrast = 30.0;
        s.effects.grain.amount = 50.0;
        let direct = RenderParams::from_settings(&s, D65_WHITE, Clipping::default());
        s.detail.sharpening.amount = 80.0;
        let staged = RenderParams::from_settings(&s, D65_WHITE, Clipping::default());
        assert_ne!(staged.flags & FLAG_SHARPEN, 0);
        let a = render(&image, &direct).rgba;
        let b = render(&image, &staged).rgba;
        // Un aplat n'a rien à accentuer : identique, grain compris.
        assert_eq!(a, b);
    }

    #[test]
    fn region_avec_marge_identique_a_l_image_entiere() {
        let (w, h) = (60u32, 50u32);
        let xyz: Vec<u16> = (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                let v = (8000 + (x * 400) % 30000 + ((x * 131 + y * 71) % 13) * 400) as u16;
                [
                    v,
                    v / 2,
                    (v / 3).saturating_add(((y * 97) % 9) as u16 * 300),
                ]
            })
            .collect();
        let raw = DevelopedRaw {
            width: w,
            height: h,
            xyz,
            as_shot_white: D65_WHITE,
            full_long_side: w,
            rendered: false,
        };
        let mut s = DevelopSettings::default();
        s.detail.noise.luminance = 80.0;
        s.detail.noise.color = 70.0;
        s.detail.sharpening.amount = 120.0;
        s.detail.sharpening.masking = 20.0;
        let p = RenderParams::from_settings(&s, D65_WHITE, Clipping::default());
        let whole = render(&WorkingImage::from_raw(&raw, w), &p).rgba;
        let (region, (cx, cy, cw, ch)) = WorkingImage::region_with_margin(
            &raw,
            &p,
            &DevelopSettings::default(),
            None,
            (25, 20, 12, 9),
        );
        let part = render(&region, &p).rgba;
        assert_eq!((cw, ch), (12, 9));
        for y in 0..ch {
            for x in 0..cw {
                let i = (((y + cy) * region.width + x + cx) * 4) as usize;
                let j = (((y + 20) * w + x + 25) * 4) as usize;
                assert_eq!(part[i..i + 3], whole[j..j + 3], "({x}, {y})");
            }
        }
    }

    #[test]
    fn region_du_zoom_avec_geometrie_identique_a_l_image_entiere() {
        let (w, h) = (90u32, 70u32);
        let xyz: Vec<u16> = (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                let v = (6000 + (x * 300 + y * 170) % 40000) as u16;
                [v, v / 2, v / 3]
            })
            .collect();
        let raw = DevelopedRaw {
            width: w,
            height: h,
            xyz,
            as_shot_white: D65_WHITE,
            full_long_side: w,
            rendered: false,
        };
        let mut s = DevelopSettings::default();
        s.geometry.angle = 6.0;
        s.geometry.quarter = 3;
        s.geometry.crop = CropRect {
            left: 0.1,
            top: 0.1,
            right: 0.9,
            bottom: 0.85,
        };
        s.detail.sharpening.amount = 100.0;
        // Dégradé sur toute l'image : chaque pixel de la région a son poids,
        // calculé dans l'image entière comme dans la région.
        s.masks = vec![Mask {
            shape: MaskShape::Linear {
                from: [0.2, 0.2],
                to: [0.8, 0.8],
            },
            adjust: LocalAdjust {
                exposure: 1.5,
                saturation: 30.0,
                ..LocalAdjust::default()
            },
            invert: false,
            hidden: false,
            strokes: Vec::new(),
        }];
        let p = RenderParams::from_settings(&s, D65_WHITE, Clipping::default()).with_masks(
            &s,
            w.into(),
            h.into(),
        );
        assert!(p.locals.is_some());
        let whole_src = WorkingImage::from_raw(&raw, w);
        let whole = geometry::transform(&whole_src, &s.geometry);
        let full = render(&whole, &p).rgba;
        let (region, (cx, cy, cw, ch)) =
            WorkingImage::region_with_margin(&raw, &p, &s, None, (14, 9, 10, 8));
        let part = render(&region, &p).rgba;
        for y in 0..ch {
            for x in 0..cw {
                let i = (((y + cy) * region.width + x + cx) * 4) as usize;
                let j = (((y + 9) * whole.width + x + 14) * 4) as usize;
                assert_eq!(part[i..i + 3], full[j..j + 3], "({x}, {y})");
            }
        }
    }

    #[test]
    fn histogramme_recalcule_identique() {
        let image = WorkingImage {
            width: 64,
            height: 8,
            rgb: (0..64 * 8 * 3).map(|i| (i % 97) as f32 / 40.0).collect(),
            as_shot_white: D65_WHITE,
            scale: 1.0,
            crop: None,
            rendered: false,
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
            scale: 1.0,
            crop: None,
            rendered: false,
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

    mod locales {
        use super::*;

        /// Image de travail unie, 40 × 20, d'un gris moyen (déjà développée :
        /// les réglages à zéro la rendent telle quelle).
        fn grise(v: f32) -> WorkingImage {
            WorkingImage {
                width: 40,
                height: 20,
                rgb: vec![v; 40 * 20 * 3],
                as_shot_white: D65_WHITE,
                scale: 1.0,
                crop: None,
                rendered: true,
            }
        }

        /// Disque net au centre (rayon 5 pixels), avec ces réglages.
        fn disque(adjust: LocalAdjust) -> Mask {
            Mask {
                shape: MaskShape::Radial {
                    center: [0.5, 0.5],
                    radius: [0.125, 0.25],
                    angle: 0.0,
                    feather: 0.0,
                },
                adjust,
                invert: false,
                hidden: false,
                strokes: Vec::new(),
            }
        }

        fn rendu(masks: Vec<Mask>, image: &WorkingImage) -> Vec<f32> {
            let s = DevelopSettings {
                masks,
                ..DevelopSettings::default()
            };
            let p = RenderParams::from_settings(&s, D65_WHITE, Clipping::default())
                .rendered(true)
                .with_masks(&s, image.width.into(), image.height.into());
            render_encoded(image, &p)
        }

        fn px(out: &[f32], x: usize, y: usize) -> [f32; 3] {
            let i = (y * 40 + x) * 3;
            [out[i], out[i + 1], out[i + 2]]
        }

        const CENTRE: (usize, usize) = (20, 10);
        const COIN: (usize, usize) = (2, 2);

        #[test]
        fn masque_sans_effet_rendu_identique() {
            let img = grise(0.18);
            let sans = rendu(Vec::new(), &img);
            let neutre = disque(LocalAdjust::default());
            assert_eq!(rendu(vec![neutre], &img), sans);
            // Masque actif : hors de sa zone, rien ne change.
            let plus = disque(LocalAdjust {
                exposure: 1.0,
                ..LocalAdjust::default()
            });
            let avec = rendu(vec![plus], &img);
            assert_eq!(px(&avec, COIN.0, COIN.1), px(&sans, COIN.0, COIN.1));
            assert!(px(&avec, CENTRE.0, CENTRE.1)[1] > px(&sans, CENTRE.0, CENTRE.1)[1]);
        }

        #[test]
        fn exposition_locale_comme_l_exposition_globale() {
            let img = grise(0.1);
            let local = rendu(
                vec![disque(LocalAdjust {
                    exposure: 1.0,
                    ..LocalAdjust::default()
                })],
                &img,
            );
            let s = DevelopSettings {
                light: crate::develop::settings::Light {
                    exposure: 1.0,
                    ..Default::default()
                },
                ..DevelopSettings::default()
            };
            let p = RenderParams::from_settings(&s, D65_WHITE, Clipping::default()).rendered(true);
            let global = render_encoded(&img, &p);
            let (a, b) = (
                px(&local, CENTRE.0, CENTRE.1),
                px(&global, CENTRE.0, CENTRE.1),
            );
            for c in 0..3 {
                assert!((a[c] - b[c]).abs() < 1e-4, "{a:?} {b:?}");
            }
        }

        #[test]
        fn deux_masques_s_additionnent() {
            let img = grise(0.05);
            let un = |ev| {
                disque(LocalAdjust {
                    exposure: ev,
                    ..LocalAdjust::default()
                })
            };
            let deux = rendu(vec![un(0.5), un(0.5)], &img);
            let seul = rendu(vec![un(1.0)], &img);
            let (a, b) = (px(&deux, CENTRE.0, CENTRE.1), px(&seul, CENTRE.0, CENTRE.1));
            assert!((a[0] - b[0]).abs() < 1e-4, "{a:?} {b:?}");
        }

        #[test]
        fn temperature_teinte_saturation_et_ombres() {
            let img = grise(0.18);
            let sans = px(&rendu(Vec::new(), &img), CENTRE.0, CENTRE.1);
            let chaud = px(
                &rendu(
                    vec![disque(LocalAdjust {
                        temperature: 60.0,
                        ..LocalAdjust::default()
                    })],
                    &img,
                ),
                CENTRE.0,
                CENTRE.1,
            );
            assert!(chaud[0] > sans[0] && chaud[2] < sans[2], "{chaud:?}");
            let magenta = px(
                &rendu(
                    vec![disque(LocalAdjust {
                        tint: 60.0,
                        ..LocalAdjust::default()
                    })],
                    &img,
                ),
                CENTRE.0,
                CENTRE.1,
            );
            assert!(magenta[1] < sans[1], "{magenta:?}");
            let ombres = px(
                &rendu(
                    vec![disque(LocalAdjust {
                        shadows: 100.0,
                        ..LocalAdjust::default()
                    })],
                    &img,
                ),
                CENTRE.0,
                CENTRE.1,
            );
            assert!(ombres[1] > sans[1] + 0.02, "{ombres:?} {sans:?}");
            // Saturation à -100 : une couleur devient grise, dans le masque seulement.
            let mut couleur = grise(0.0);
            for c in couleur.rgb.as_chunks_mut::<3>().0 {
                *c = [0.3, 0.1, 0.05];
            }
            let gris = rendu(
                vec![disque(LocalAdjust {
                    saturation: -100.0,
                    ..LocalAdjust::default()
                })],
                &couleur,
            );
            let dedans = px(&gris, CENTRE.0, CENTRE.1);
            let dehors = px(&gris, COIN.0, COIN.1);
            assert!((dedans[0] - dedans[2]).abs() < 1e-3, "{dedans:?}");
            assert!(dehors[0] - dehors[2] > 0.2, "{dehors:?}");
        }

        #[test]
        fn aussi_dans_le_rendu_par_passes() {
            // Réduction du bruit : le rendu passe par l'image linéaire entière.
            let img = grise(0.1);
            let mut s = DevelopSettings {
                masks: vec![disque(LocalAdjust {
                    exposure: 1.0,
                    ..LocalAdjust::default()
                })],
                ..DevelopSettings::default()
            };
            s.detail.noise.luminance = 30.0;
            let p = RenderParams::from_settings(&s, D65_WHITE, Clipping::default())
                .rendered(true)
                .with_masks(&s, img.width.into(), img.height.into());
            assert!(p.flags & FLAG_DENOISE != 0);
            let out = render_encoded(&img, &p);
            assert!(px(&out, CENTRE.0, CENTRE.1)[1] > px(&out, COIN.0, COIN.1)[1] + 0.05);
        }
    }
}
