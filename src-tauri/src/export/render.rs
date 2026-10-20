//! Rendu d'export : l'original en pleine définition (ou en demi-définition
//! quand elle suffit à la taille demandée), corrigé de l'objectif et des
//! défauts, transformé par la géométrie, réduit à la taille exacte, puis
//! rendu par le pipeline (processeur) en virgule flottante, accentué pour la
//! sortie et converti dans l'espace choisi.

use std::thread;

use serde::{Deserialize, Serialize};

use super::ExportError;
use super::color::ColorSpace;
use super::watermark::{self, Watermark};
use crate::develop::geometry;
use crate::develop::lens::{LensCorrection, LensProfile};
use crate::develop::pipeline::{Clipping, RenderParams, WorkingImage, render_encoded};
use crate::develop::settings::DevelopSettings;
use crate::develop::spots;
use crate::raw::RawFile;

/// Taille de sortie ; jamais d'agrandissement au-delà de l'original.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode", content = "value")]
pub enum Resize {
    /// Pleine résolution.
    #[default]
    Full,
    LongSide(u32),
    Width(u32),
    Height(u32),
    /// Pourcentage de la pleine résolution.
    Percent(f64),
    /// Tenir dans un cadre largeur × hauteur (Instagram : 1080 × 1350).
    Fit([u32; 2]),
}

/// Accentuation de sortie, d'après le support.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sharpen {
    #[default]
    None,
    Screen,
    Paper,
}

/// Taille de sortie pour une image rendue de `w` × `h` en pleine résolution.
pub fn target_size(w: u32, h: u32, resize: Resize) -> (u32, u32) {
    let (wf, hf) = (f64::from(w.max(1)), f64::from(h.max(1)));
    let k = match resize {
        Resize::Full => 1.0,
        Resize::LongSide(n) => f64::from(n) / wf.max(hf),
        Resize::Width(n) => f64::from(n) / wf,
        Resize::Height(n) => f64::from(n) / hf,
        Resize::Percent(p) => p / 100.0,
        Resize::Fit([bw, bh]) => (f64::from(bw) / wf).min(f64::from(bh) / hf),
    };
    let k = if k.is_finite() && k > 0.0 {
        k.min(1.0)
    } else {
        1.0
    };
    (
        ((wf * k).round() as u32).clamp(1, w.max(1)),
        ((hf * k).round() as u32).clamp(1, h.max(1)),
    )
}

/// Image rendue, valeurs codées dans l'espace de sortie (0 à 1).
#[derive(Debug, Clone)]
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<f32>,
}

impl Rendered {
    pub fn to_u8(&self) -> Vec<u8> {
        self.rgb
            .iter()
            .map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
            .collect()
    }

    pub fn to_u16(&self) -> Vec<u16> {
        self.rgb
            .iter()
            .map(|v| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16)
            .collect()
    }
}

/// Options de rendu d'une photo.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions {
    pub resize: Resize,
    pub sharpen: Sharpen,
    pub color: ColorSpace,
}

/// Dimensions de l'image décodée, orientation du boîtier appliquée.
fn oriented_size(raw: &RawFile) -> (u32, u32) {
    let i = raw.info();
    let (w, h) = (u32::from(i.width), u32::from(i.height));
    if (5..=8).contains(&i.orientation) {
        (h, w)
    } else {
        (w, h)
    }
}

/// Rend une photo avec ses réglages, et le filigrane s'il y en a un.
pub fn render_photo(
    raw: &mut RawFile,
    settings: &DevelopSettings,
    profile: Option<&LensProfile>,
    options: RenderOptions,
    mark: Option<&Watermark>,
) -> Result<Rendered, ExportError> {
    let g = &settings.geometry;
    // Demi-définition quand la sortie n'en demande pas plus.
    let (fw, fh) = oriented_size(raw);
    let full = geometry::plan(g, f64::from(fw), f64::from(fh));
    let (tw, _) = target_size(full.width, full.height, options.resize);
    let half = f64::from(tw) <= f64::from(full.width) * 0.48;
    let developed = raw.develop(half)?;

    // Image de travail juste assez grande pour la sortie.
    let out = geometry::plan(g, f64::from(developed.width), f64::from(developed.height));
    let (tw, th) = target_size(out.width, out.height, options.resize);
    let k = f64::from(tw) / f64::from(out.width.max(1));
    let long = developed.width.max(developed.height);
    let side = ((f64::from(long) * k).ceil() as u32 + 2).min(long);
    let base = WorkingImage::from_raw(&developed, side);
    drop(developed);

    let corrected = match profile.and_then(|p| LensCorrection::new(p, &settings.lens)) {
        Some(c) => c.for_image(base.width, base.height).whole(&base),
        None => base,
    };
    let healed = spots::healed(&corrected, &settings.spots);
    drop(corrected);
    let shaped = geometry::transform(&healed, g);
    drop(healed);
    let sized = if (shaped.width, shaped.height) == (tw, th) {
        shaped
    } else {
        shaped.resized(tw, th)
    };

    let params = RenderParams::from_settings(settings, sized.as_shot_white, Clipping::default());
    let mut rgb = render_encoded(&sized, &params);
    sharpen(&mut rgb, tw as usize, th as usize, options.sharpen);
    let mut out = Rendered {
        width: tw,
        height: th,
        rgb,
    };
    if let Some(mark) = mark {
        watermark::apply(&mut out, mark)?;
    }
    options.color.convert(&mut out.rgb)?;
    Ok(out)
}

/// Flou gaussien séparable d'un plan, bords répétés.
fn blur(plane: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil() as isize;
    let kernel: Vec<f32> = (-r..=r)
        .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f32 = kernel.iter().sum();
    let kernel: Vec<f32> = kernel.iter().map(|k| k / sum).collect();
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0.0f32; w * h];
        let rows = h.div_ceil(crate::develop::pipeline::workers()).max(1);
        thread::scope(|s| {
            for (k, chunk) in out.chunks_mut(rows * w).enumerate() {
                let kernel = &kernel;
                s.spawn(move || {
                    for (i, v) in chunk.iter_mut().enumerate() {
                        let index = k * rows * w + i;
                        let (x, y) = ((index % w) as isize, (index / w) as isize);
                        let mut acc = 0.0;
                        for (j, kv) in kernel.iter().enumerate() {
                            let d = j as isize - r;
                            let (xx, yy) = if horizontal {
                                ((x + d).clamp(0, w as isize - 1), y)
                            } else {
                                (x, (y + d).clamp(0, h as isize - 1))
                            };
                            acc += src[yy as usize * w + xx as usize] * kv;
                        }
                        *v = acc;
                    }
                });
            }
        });
        out
    };
    pass(&pass(plane, true), false)
}

/// Accentuation de sortie : masque flou sur la luminance, ajouté aux trois
/// canaux (aucune frange colorée).
fn sharpen(rgb: &mut [f32], w: usize, h: usize, kind: Sharpen) {
    let (sigma, amount) = match kind {
        Sharpen::None => return,
        Sharpen::Screen => (0.6, 0.35),
        Sharpen::Paper => (1.0, 0.6),
    };
    if w < 3 || h < 3 {
        return;
    }
    let luma: Vec<f32> = rgb
        .chunks(3)
        .map(|p| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
        .collect();
    let blurred = blur(&luma, w, h, sigma);
    for ((px, l), b) in rgb.chunks_mut(3).zip(&luma).zip(&blurred) {
        let d = (l - b) * amount;
        for v in px {
            *v = (*v + d).clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailles_de_sortie() {
        assert_eq!(target_size(6000, 4000, Resize::Full), (6000, 4000));
        assert_eq!(
            target_size(6000, 4000, Resize::LongSide(2048)),
            (2048, 1365)
        );
        assert_eq!(
            target_size(4000, 6000, Resize::LongSide(2048)),
            (1365, 2048)
        );
        assert_eq!(target_size(6000, 4000, Resize::Width(1000)), (1000, 667));
        assert_eq!(target_size(6000, 4000, Resize::Height(1000)), (1500, 1000));
        assert_eq!(target_size(6000, 4000, Resize::Percent(50.0)), (3000, 2000));
        // Instagram : 1080 × 1350 au plus.
        assert_eq!(
            target_size(6000, 4000, Resize::Fit([1080, 1350])),
            (1080, 720)
        );
        assert_eq!(
            target_size(4000, 6000, Resize::Fit([1080, 1350])),
            (900, 1350)
        );
        // Jamais d'agrandissement.
        assert_eq!(target_size(800, 600, Resize::LongSide(2048)), (800, 600));
        assert_eq!(target_size(800, 600, Resize::Percent(f64::NAN)), (800, 600));
    }

    #[test]
    fn accentuation_renforce_un_bord_sans_toucher_aux_aplats() {
        let (w, h) = (20usize, 10usize);
        let mut rgb: Vec<f32> = (0..w * h)
            .flat_map(|i| {
                let v = if i % w < 10 { 0.3 } else { 0.7 };
                [v, v, v]
            })
            .collect();
        let before = rgb.clone();
        sharpen(&mut rgb, w, h, Sharpen::Paper);
        let at = |v: &[f32], x: usize| v[(5 * w + x) * 3];
        assert!(at(&rgb, 9) < at(&before, 9));
        assert!(at(&rgb, 10) > at(&before, 10));
        assert!((at(&rgb, 1) - 0.3).abs() < 1e-4);
        let mut none = before.clone();
        sharpen(&mut none, w, h, Sharpen::None);
        assert_eq!(none, before);
    }
}
