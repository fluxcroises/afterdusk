//! Réduction du bruit et netteté (processus 1) : les seules étapes qui
//! regardent les pixels voisins. Référence processeur ; le GPU (`gpu.rs`)
//! refait les mêmes passes dans le même ordre.
//!
//! - Bruit de couleur : les écarts de couleur (R − Y, B − Y) sont moyennés sur
//!   un large voisinage ; la luminance n'est pas touchée.
//! - Bruit de luminance : filtre guidé (He et al.) sur la luminance perçue,
//!   qui lisse les zones unies et garde les contours ; appliqué en rapport sur
//!   les trois canaux pour garder la couleur.
//! - Netteté : masque flou sur la luminance de l'image rendue, halos retenus
//!   (détail) et zones unies épargnées (masquage).
//!
//! Les rayons sont définis en pixels de l'original et ramenés à l'échelle de
//! l'image rendue : l'aperçu, le zoom et l'export s'accordent.

use std::thread;

use super::pipeline::LUMA_REC2020;
use super::settings::Detail;

/// Constantes figées du processus 1.
pub mod process1 {
    /// Rayon du filtre guidé, en pixels de l'original.
    pub const NOISE_LUMA_RADIUS: f32 = 2.0;
    /// Écart type du bruit lissé à 100, en luminance perçue (0 à 1).
    pub const NOISE_LUMA_EPS: f32 = 0.035;
    /// Rayon de la moyenne des couleurs, en pixels de l'original.
    pub const NOISE_COLOR_RADIUS: f32 = 4.0;
    /// Netteté : seuil des halos, de détail 0 à détail 100.
    pub const SHARPEN_LIMIT_MIN: f32 = 0.02;
    pub const SHARPEN_LIMIT_RANGE: f32 = 0.3;
    /// Netteté : pente (gradient local) sous laquelle masquage 100 épargne.
    pub const SHARPEN_MASK_EDGE: f32 = 0.05;
    /// Plus petit écart type du flou de netteté, en pixels de l'image rendue.
    pub const SHARPEN_SIGMA_MIN: f32 = 0.25;
}

/// Réglages de détail ramenés de 0 à 1 (la quantité de netteté de 0 à 1,5).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DetailParams {
    pub luminance: f32,
    pub luminance_detail: f32,
    pub color: f32,
    pub sharpen: f32,
    /// Rayon de la netteté, en pixels de l'original.
    pub radius: f32,
    pub sharpen_detail: f32,
    pub masking: f32,
}

impl DetailParams {
    pub fn from_settings(d: &Detail) -> Self {
        let pct = |v: f64| (v / 100.0) as f32;
        Self {
            luminance: pct(d.noise.luminance),
            luminance_detail: pct(d.noise.luminance_detail),
            color: pct(d.noise.color),
            sharpen: pct(d.sharpening.amount),
            radius: d.sharpening.radius as f32,
            sharpen_detail: pct(d.sharpening.detail),
            masking: pct(d.sharpening.masking),
        }
    }

    pub fn denoises(&self) -> bool {
        self.luminance > 0.0 || self.color > 0.0
    }

    pub fn sharpens(&self) -> bool {
        self.sharpen > 0.0
    }

    /// Lissage du filtre guidé : carré de l'écart type du bruit visé.
    pub fn eps(&self) -> f32 {
        let s = process1::NOISE_LUMA_EPS * self.luminance;
        s * s * (1.5 - self.luminance_detail)
    }

    /// Seuil des halos de la netteté.
    pub fn sharpen_limit(&self) -> f32 {
        process1::SHARPEN_LIMIT_MIN + process1::SHARPEN_LIMIT_RANGE * self.sharpen_detail
    }
}

/// Rayons à l'échelle de l'image rendue (`scale` : voir `WorkingImage::scale`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radii {
    pub luminance: u32,
    pub color: u32,
    pub sigma: f32,
    /// Demi-largeur du noyau gaussien de la netteté.
    pub kernel: u32,
}

impl Radii {
    pub fn at(p: &DetailParams, scale: f32) -> Self {
        let px = |r: f32| ((r * scale).round() as u32).max(1);
        let sigma = (p.radius * scale).max(process1::SHARPEN_SIGMA_MIN);
        Self {
            luminance: px(process1::NOISE_LUMA_RADIUS),
            color: px(process1::NOISE_COLOR_RADIUS),
            sigma,
            kernel: ((3.0 * sigma).ceil() as u32).max(1),
        }
    }

    /// Marge à rendre autour d'une région pour que ses bords soient justes.
    pub fn margin(&self) -> u32 {
        2 * self.luminance + self.color + self.kernel + 1
    }
}

fn workers() -> usize {
    thread::available_parallelism().map_or(4, |n| n.get())
}

/// Applique `f(y, ligne)` à chaque ligne de `out` (`row` valeurs par ligne),
/// réparti sur les cœurs.
fn par_rows(out: &mut [f32], row: usize, f: impl Fn(usize, &mut [f32]) + Sync) {
    let rows = out.len() / row.max(1);
    let per = rows.div_ceil(workers()).max(1);
    thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(per * row).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (r, line) in chunk.chunks_mut(row).enumerate() {
                    f(k * per + r, line);
                }
            });
        }
    });
}

/// Moyenne sur un carré de côté 2r + 1 (séparable : lignes puis colonnes),
/// sur les seuls pixels de l'image près des bords. `ch` valeurs par pixel.
pub fn box_blur(buf: &[f32], w: usize, h: usize, ch: usize, r: usize) -> Vec<f32> {
    let mut tmp = vec![0.0f32; buf.len()];
    par_rows(&mut tmp, w * ch, |y, line| {
        for x in 0..w {
            let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
            let n = (hi - lo + 1) as f32;
            for c in 0..ch {
                let mut sum = 0.0f32;
                for k in lo..=hi {
                    sum += buf[(y * w + k) * ch + c];
                }
                line[x * ch + c] = sum / n;
            }
        }
    });
    let mut out = vec![0.0f32; buf.len()];
    par_rows(&mut out, w * ch, |y, line| {
        let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
        let n = (hi - lo + 1) as f32;
        for x in 0..w {
            for c in 0..ch {
                let mut sum = 0.0f32;
                for k in lo..=hi {
                    sum += tmp[(k * w + x) * ch + c];
                }
                line[x * ch + c] = sum / n;
            }
        }
    });
    out
}

/// Flou gaussien d'un canal (séparable), poids renormalisés près des bords.
pub fn gaussian_blur(buf: &[f32], w: usize, h: usize, sigma: f32, kernel: usize) -> Vec<f32> {
    let weight = |k: isize| (-((k * k) as f32) / (2.0 * sigma * sigma)).exp();
    let mut tmp = vec![0.0f32; buf.len()];
    par_rows(&mut tmp, w, |y, line| {
        for (x, out) in line.iter_mut().enumerate() {
            let (mut sum, mut total) = (0.0f32, 0.0f32);
            for k in -(kernel as isize)..=kernel as isize {
                let xx = x as isize + k;
                if xx >= 0 && (xx as usize) < w {
                    let wt = weight(k);
                    sum += buf[y * w + xx as usize] * wt;
                    total += wt;
                }
            }
            *out = sum / total;
        }
    });
    let mut out = vec![0.0f32; buf.len()];
    par_rows(&mut out, w, |y, line| {
        for (x, out) in line.iter_mut().enumerate() {
            let (mut sum, mut total) = (0.0f32, 0.0f32);
            for k in -(kernel as isize)..=kernel as isize {
                let yy = y as isize + k;
                if yy >= 0 && (yy as usize) < h {
                    let wt = weight(k);
                    sum += tmp[yy as usize * w + x] * wt;
                    total += wt;
                }
            }
            *out = sum / total;
        }
    });
    out
}

#[inline]
fn luma(c: &[f32]) -> f32 {
    LUMA_REC2020[0] * c[0] + LUMA_REC2020[1] * c[1] + LUMA_REC2020[2] * c[2]
}

/// Réduction du bruit sur l'image linéaire (Rec. 2020, trois valeurs par pixel).
pub fn denoise(lin: &mut [f32], w: usize, h: usize, p: &DetailParams, r: &Radii) {
    if p.color > 0.0 {
        let mut chroma = vec![0.0f32; w * h * 2];
        for (i, px) in lin.as_chunks::<3>().0.iter().enumerate() {
            let y = luma(px);
            chroma[i * 2] = px[0] - y;
            chroma[i * 2 + 1] = px[2] - y;
        }
        let blurred = box_blur(&chroma, w, h, 2, r.color as usize);
        for (i, px) in lin.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            let y = luma(px);
            let cr = chroma[i * 2] + (blurred[i * 2] - chroma[i * 2]) * p.color;
            let cb = chroma[i * 2 + 1] + (blurred[i * 2 + 1] - chroma[i * 2 + 1]) * p.color;
            let red = y + cr;
            let blue = y + cb;
            px[0] = red;
            px[2] = blue;
            px[1] = (y - LUMA_REC2020[0] * red - LUMA_REC2020[2] * blue) / LUMA_REC2020[1];
        }
    }
    if p.luminance > 0.0 {
        let gamma = super::pipeline::process1::TONE_GAMMA;
        let mut guide = vec![0.0f32; w * h * 2];
        for (i, px) in lin.as_chunks::<3>().0.iter().enumerate() {
            let v = luma(px).max(0.0).powf(1.0 / gamma);
            guide[i * 2] = v;
            guide[i * 2 + 1] = v * v;
        }
        let means = box_blur(&guide, w, h, 2, r.luminance as usize);
        let eps = p.eps();
        let mut coef = vec![0.0f32; w * h * 2];
        for i in 0..w * h {
            let (m, mm) = (means[i * 2], means[i * 2 + 1]);
            let var = (mm - m * m).max(0.0);
            let a = var / (var + eps);
            coef[i * 2] = a;
            coef[i * 2 + 1] = m * (1.0 - a);
        }
        let smooth = box_blur(&coef, w, h, 2, r.luminance as usize);
        for (i, px) in lin.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            let y = luma(px);
            if y > 1e-6 {
                let q = (smooth[i * 2] * guide[i * 2] + smooth[i * 2 + 1]).max(0.0);
                let ratio = q.powf(gamma) / y;
                for c in px.iter_mut() {
                    *c *= ratio;
                }
            }
        }
    }
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Luminance des valeurs codées (coefficients Rec. 709), pour la netteté.
#[inline]
pub fn encoded_luma(c: &[f32]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Masque flou sur l'image rendue (valeurs codées, trois par pixel).
pub fn sharpen(enc: &mut [f32], w: usize, h: usize, p: &DetailParams, r: &Radii) {
    let l: Vec<f32> = enc
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| encoded_luma(c))
        .collect();
    let blurred = gaussian_blur(&l, w, h, r.sigma, r.kernel as usize);
    let limit = p.sharpen_limit();
    let edge = process1::SHARPEN_MASK_EDGE * p.masking;
    for (i, px) in enc.as_chunks_mut::<3>().0.iter_mut().enumerate() {
        let (x, y) = (i % w, i / w);
        let d = l[i] - blurred[i];
        let soft = d / (1.0 + d.abs() / limit);
        let mask = if p.masking > 0.0 {
            let at = |xx: usize, yy: usize| blurred[yy * w + xx];
            let gx = at((x + 1).min(w - 1), y) - at(x.saturating_sub(1), y);
            let gy = at(x, (y + 1).min(h - 1)) - at(x, y.saturating_sub(1));
            smoothstep(0.5 * edge, 1.5 * edge, 0.5 * gx.hypot(gy))
        } else {
            1.0
        };
        let delta = p.sharpen * soft * mask;
        for c in px.iter_mut() {
            *c += delta;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flou_de_surface_garde_la_moyenne_et_les_aplats() {
        let (w, h) = (9, 7);
        let flat = vec![0.4f32; w * h];
        assert!(
            box_blur(&flat, w, h, 1, 3)
                .iter()
                .all(|v| (v - 0.4).abs() < 1e-6)
        );
        let g = gaussian_blur(&flat, w, h, 1.2, 4);
        assert!(g.iter().all(|v| (v - 0.4).abs() < 1e-6));
        // Un point isolé s'étale sans changer la somme loin des bords.
        let mut dot = vec![0.0f32; 21 * 21];
        dot[10 * 21 + 10] = 1.0;
        let b = box_blur(&dot, 21, 21, 1, 2);
        assert!((b.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert!((b[10 * 21 + 10] - 1.0 / 25.0).abs() < 1e-6);
    }

    fn noisy(w: usize, h: usize, amp: f32) -> Vec<f32> {
        // Aplat gris bruité (luminance et couleur), puis un bord net au milieu.
        let mut v = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let base = if x < w / 2 { 0.1 } else { 0.4 };
                let n = |k: usize| {
                    let s = ((x * 7919 + y * 104_729 + k * 31) % 1000) as f32 / 1000.0 - 0.5;
                    s * amp
                };
                v.extend_from_slice(&[base + n(0), base + n(1), base + n(2)]);
            }
        }
        v
    }

    fn spread(buf: &[f32], w: usize, x0: usize, x1: usize, h: usize) -> f32 {
        let vals: Vec<f32> = (0..h)
            .flat_map(|y| (x0..x1).map(move |x| (y, x)))
            .map(|(y, x)| luma(&buf[(y * w + x) * 3..(y * w + x) * 3 + 3]))
            .collect();
        let mean = vals.iter().sum::<f32>() / vals.len() as f32;
        (vals.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / vals.len() as f32).sqrt()
    }

    #[test]
    fn bruit_de_luminance_lisse_les_aplats_garde_le_bord() {
        let (w, h) = (40, 30);
        let mut lin = noisy(w, h, 0.04);
        let before = spread(&lin, w, 2, 15, h);
        let p = DetailParams {
            luminance: 1.0,
            luminance_detail: 0.5,
            ..Default::default()
        };
        denoise(&mut lin, w, h, &p, &Radii::at(&p, 1.0));
        let after = spread(&lin, w, 2, 15, h);
        assert!(after < before * 0.6, "{before} → {after}");
        // Le bord reste net : grand écart entre les deux côtés.
        let left = luma(&lin[(15 * w + 17) * 3..(15 * w + 17) * 3 + 3]);
        let right = luma(&lin[(15 * w + 22) * 3..(15 * w + 22) * 3 + 3]);
        assert!(right - left > 0.25, "{left} {right}");
    }

    #[test]
    fn bruit_de_couleur_garde_la_luminance() {
        let (w, h) = (30, 20);
        let mut lin = noisy(w, h, 0.06);
        let lum_before: Vec<f32> = lin.as_chunks::<3>().0.iter().map(|c| luma(c)).collect();
        let chroma = |b: &[f32]| {
            b.as_chunks::<3>()
                .0
                .iter()
                .map(|c| (c[0] - luma(c)).abs() + (c[2] - luma(c)).abs())
                .sum::<f32>()
        };
        let c_before = chroma(&lin);
        let p = DetailParams {
            color: 1.0,
            ..Default::default()
        };
        denoise(&mut lin, w, h, &p, &Radii::at(&p, 1.0));
        assert!(chroma(&lin) < c_before * 0.5);
        for (i, c) in lin.as_chunks::<3>().0.iter().enumerate() {
            assert!((luma(c) - lum_before[i]).abs() < 1e-5);
        }
    }

    #[test]
    fn nettete_renforce_le_bord_et_respecte_le_masquage() {
        let (w, h) = (20, 10);
        let mut enc = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let v = if x < w / 2 { 0.3 } else { 0.6 };
                enc.extend_from_slice(&[v, v, v]);
            }
        }
        let orig = enc.clone();
        let p = DetailParams {
            sharpen: 1.0,
            radius: 1.0,
            sharpen_detail: 1.0,
            ..Default::default()
        };
        sharpen(&mut enc, w, h, &p, &Radii::at(&p, 1.0));
        let at = |b: &[f32], x: usize| b[(5 * w + x) * 3];
        // Plus sombre juste avant le bord, plus clair juste après ; aplats intacts.
        assert!(at(&enc, w / 2 - 1) < at(&orig, w / 2 - 1));
        assert!(at(&enc, w / 2) > at(&orig, w / 2));
        assert!((at(&enc, 1) - 0.3).abs() < 1e-5);
        // Détail 0 : halos retenus, effet plus faible au bord.
        let mut soft = orig.clone();
        let q = DetailParams {
            sharpen_detail: 0.0,
            ..p
        };
        sharpen(&mut soft, w, h, &q, &Radii::at(&q, 1.0));
        assert!(at(&soft, w / 2) - 0.6 < at(&enc, w / 2) - 0.6);
        // Masquage : un aplat légèrement bruité n'est pas accentué.
        let mut grain: Vec<f32> = (0..w * h)
            .flat_map(|i| {
                let v = 0.5 + ((i * 37) % 7) as f32 * 0.002;
                [v, v, v]
            })
            .collect();
        let reference = grain.clone();
        let m = DetailParams { masking: 1.0, ..p };
        sharpen(&mut grain, w, h, &m, &Radii::at(&m, 1.0));
        let moved: f32 = grain
            .iter()
            .zip(&reference)
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(moved < 1e-3, "{moved}");
    }

    #[test]
    fn rayons_a_l_echelle_et_marge() {
        let p = DetailParams {
            sharpen: 0.5,
            radius: 2.0,
            ..Default::default()
        };
        let full = Radii::at(&p, 1.0);
        let preview = Radii::at(&p, 0.33);
        assert_eq!((full.luminance, full.color), (2, 4));
        assert_eq!((preview.luminance, preview.color), (1, 1));
        assert!((preview.sigma - 0.66).abs() < 1e-6);
        assert_eq!(full.margin(), 2 * 2 + 4 + 6 + 1);
    }
}
