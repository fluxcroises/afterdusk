//! Réglages automatiques (cahier des charges §4.5) : calcul local et
//! déterministe, dont le résultat n'est qu'un jeu de valeurs ordinaires.
//!
//! Tonalité : on rend une vignette avec le pipeline lui-même et on lit
//! l'histogramme de luminance de l'image rendue ; chaque curseur est ensuite
//! cherché par dichotomie pour atteindre sa cible, dans l'ordre exposition,
//! hautes lumières, ombres, blancs, noirs, puis un peu de contraste si l'image
//! est terne. Ce que l'utilisateur voit est donc exactement ce qui est mesuré.
//!
//! Balance des blancs : première estimation sur les tons moyens peu saturés
//! (« monde gris »), puis affinée sur les seuls pixels que cette estimation
//! rend presque neutres ; sans gris fiable, la balance du boîtier reste.

use super::pipeline::{Clipping, RenderParams, WorkingImage, render};
use super::settings::{DevelopSettings, Light, WhiteBalance};
use super::white;
use crate::raw::D65_WHITE;

/// Côté long de la vignette mesurée.
const SAMPLE_SIDE: u32 = 512;
/// Pas de la dichotomie.
const STEPS: usize = 12;

/// Cibles, en niveaux de luminance sur 255 et en parts de l'image.
const MEDIAN_TARGET: f64 = 110.0;
const HIGHLIGHT_LEVEL: usize = 250;
const HIGHLIGHT_MAX_SHARE: f64 = 0.01;
const HIGHLIGHT_TARGET_SHARE: f64 = 0.005;
const SHADOW_LEVEL: usize = 12;
const SHADOW_MAX_SHARE: f64 = 0.03;
const SHADOW_TARGET_SHARE: f64 = 0.015;
const WHITE_POINT_MIN: f64 = 240.0;
const WHITE_POINT_TARGET: f64 = 245.0;
const BLACK_POINT_MAX: f64 = 10.0;
const BLACK_POINT_TARGET: f64 = 6.0;
const FLAT_SPREAD: f64 = 170.0;
/// Bornes des curseurs blancs et noirs automatiques : au-delà, l'image est
/// forcée plutôt qu'optimisée.
const WHITES_MAX: f64 = 50.0;
/// Baisse d'exposition au plus, pour sauver les hautes lumières d'une scène
/// très contrastée.
const MAX_EXPOSURE_DROP: f64 = 1.0;
/// Part des tons moyens qui doit paraître neutre pour croire à l'estimation.
const MIN_NEUTRAL_SHARE: f64 = 0.02;
const BLACKS_MIN: f64 = -50.0;
/// Les lumières réelles sont proches du lieu de Planck (|Duv| ≤ 0,01) : une
/// estimation plus teintée est rejetée.
const AUTO_TINT_LIMIT: f64 = 30.0;
/// Pixels candidats au gris : à cette distance (CIE 1960) du neutre du boîtier.
const CANDIDATE_DISTANCE: f64 = 0.06;

/// Vignette prélevée régulièrement dans l'image de travail.
fn sample(image: &WorkingImage) -> WorkingImage {
    let long = image.width.max(image.height).max(1);
    let stride = long.div_ceil(SAMPLE_SIDE).max(1);
    let (w, h) = (image.width / stride, image.height / stride);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let i = (((y * stride) * image.width + x * stride) * 3) as usize;
            rgb.extend_from_slice(&image.rgb[i..i + 3]);
        }
    }
    WorkingImage {
        width: w,
        height: h,
        rgb,
        as_shot_white: image.as_shot_white,
    }
}

/// Histogramme de luminance (256 cases) de la vignette rendue.
struct Luma([u32; 256], f64);

impl Luma {
    fn of(image: &WorkingImage, settings: &DevelopSettings) -> Self {
        let p = RenderParams::from_settings(settings, image.as_shot_white, Clipping::default());
        let h = render(image, &p).histogram;
        let mut bins = [0u32; 256];
        bins.copy_from_slice(&h[768..1024]);
        let total = bins.iter().map(|v| f64::from(*v)).sum::<f64>().max(1.0);
        Self(bins, total)
    }

    /// Niveau sous lequel se trouve la part `q` des pixels.
    fn percentile(&self, q: f64) -> f64 {
        let mut acc = 0.0;
        for (level, n) in self.0.iter().enumerate() {
            acc += f64::from(*n);
            if acc >= q * self.1 {
                return level as f64;
            }
        }
        255.0
    }

    /// Part des pixels à `level` ou au-dessus (`above`), ou en dessous.
    fn share(&self, level: usize, above: bool) -> f64 {
        let range = if above { level..256 } else { 0..level + 1 };
        self.0[range].iter().map(|v| f64::from(*v)).sum::<f64>() / self.1
    }
}

/// Dichotomie : valeur de `[lo, hi]` pour laquelle `metric` atteint
/// `target`, `metric` étant monotone (croissante si `increasing`).
fn solve(lo: f64, hi: f64, target: f64, increasing: bool, metric: impl Fn(f64) -> f64) -> f64 {
    let (mut lo, mut hi) = (lo, hi);
    for _ in 0..STEPS {
        let mid = (lo + hi) / 2.0;
        if (metric(mid) < target) == increasing {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// Section Lumière automatique, à partir des réglages actuels (balance,
/// couleur et luminosité gardées ; les autres curseurs de lumière recalculés).
pub fn auto_light(image: &WorkingImage, base: &DevelopSettings) -> Light {
    let small = sample(image);
    let mut s = base.clone();
    s.light = Light {
        brightness: base.light.brightness,
        ..Light::default()
    };
    let measure = |s: &DevelopSettings| Luma::of(&small, s);
    let with = |s: &DevelopSettings, f: &dyn Fn(&mut Light)| {
        let mut s = s.clone();
        f(&mut s.light);
        s
    };

    // Exposition : la médiane au gris moyen.
    let ev = solve(-3.0, 3.0, MEDIAN_TARGET, true, |v| {
        measure(&with(&s, &|l| l.exposure = v)).percentile(0.5)
    });
    s.light.exposure = (ev * 20.0).round() / 20.0;

    // Hautes lumières : peu de pixels brûlés.
    if measure(&s).share(HIGHLIGHT_LEVEL, true) > HIGHLIGHT_MAX_SHARE {
        let v = solve(-80.0, 0.0, HIGHLIGHT_TARGET_SHARE, true, |v| {
            measure(&with(&s, &|l| l.highlights = v)).share(HIGHLIGHT_LEVEL, true)
        });
        s.light.highlights = v.round();
        // Toujours trop de blanc brûlé : l'exposition redescend un peu, les
        // ombres relèveront ensuite le reste de l'image.
        if measure(&s).share(HIGHLIGHT_LEVEL, true) > 2.0 * HIGHLIGHT_MAX_SHARE {
            let ev = solve(
                s.light.exposure - MAX_EXPOSURE_DROP,
                s.light.exposure,
                HIGHLIGHT_MAX_SHARE,
                true,
                |v| measure(&with(&s, &|l| l.exposure = v)).share(HIGHLIGHT_LEVEL, true),
            );
            s.light.exposure = (ev * 20.0).round() / 20.0;
        }
    }

    // Scène très contrastée : l'exposition a baissé pour le ciel, les ombres
    // ramènent le reste de l'image au gris moyen.
    if measure(&s).percentile(0.5) < MEDIAN_TARGET - 15.0 {
        let v = solve(0.0, 80.0, MEDIAN_TARGET, true, |v| {
            measure(&with(&s, &|l| l.shadows = v)).percentile(0.5)
        });
        s.light.shadows = v.round();
    }

    // Ombres : peu de pixels bouchés.
    if s.light.shadows == 0.0 && measure(&s).share(SHADOW_LEVEL, false) > SHADOW_MAX_SHARE {
        let v = solve(0.0, 60.0, SHADOW_TARGET_SHARE, false, |v| {
            measure(&with(&s, &|l| l.shadows = v)).share(SHADOW_LEVEL, false)
        });
        s.light.shadows = v.round();
    }

    // Blancs : le haut de l'histogramme jusqu'au blanc.
    if measure(&s).percentile(0.995) < WHITE_POINT_MIN {
        let v = solve(0.0, WHITES_MAX, WHITE_POINT_TARGET, true, |v| {
            measure(&with(&s, &|l| l.whites = v)).percentile(0.995)
        });
        s.light.whites = v.round();
    }

    // Noirs : le bas de l'histogramme jusqu'au noir.
    if measure(&s).percentile(0.005) > BLACK_POINT_MAX {
        let v = solve(BLACKS_MIN, 0.0, BLACK_POINT_TARGET, true, |v| {
            measure(&with(&s, &|l| l.blacks = v)).percentile(0.005)
        });
        s.light.blacks = v.round();
    }

    // Contraste : un peu de relief pour une image terne, jamais d'aplatissement.
    let m = measure(&s);
    let spread = m.percentile(0.95) - m.percentile(0.05);
    if spread < FLAT_SPREAD {
        s.light.contrast = ((FLAT_SPREAD - spread) / 2.0).min(30.0).round();
    }
    s.light
}

/// Distance chromatique (CIE 1960) entre deux couleurs XYZ.
fn uv_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let uv = |c: [f64; 3]| {
        let d = c[0] + 15.0 * c[1] + 3.0 * c[2];
        [4.0 * c[0] / d, 6.0 * c[1] / d]
    };
    let (p, q) = (uv(a), uv(b));
    (p[0] - q[0]).hypot(p[1] - q[1])
}

/// Balance du boîtier, en valeurs (pour que l'interface affiche ce qu'Auto a gardé).
fn as_shot(image: &WorkingImage) -> WhiteBalance {
    let (temperature, tint) = white::temperature_of(white::xy_of(image.as_shot_white));
    WhiteBalance::Custom {
        temperature: temperature.round(),
        tint: tint.round(),
    }
}

/// Balance des blancs automatique.
pub fn auto_white(image: &WorkingImage) -> WhiteBalance {
    let small = sample(image);
    let to_xyz = white::invert(&super::pipeline::XYZ_TO_REC2020.map(|r| r.map(f64::from)));
    let pixels: Vec<[f64; 3]> = small
        .rgb
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| white::apply(&to_xyz, p.map(f64::from)))
        .filter(|c| c.iter().all(|v| v.is_finite()) && c[1] > 0.0)
        .collect();
    if pixels.is_empty() {
        return WhiteBalance::AsShot;
    }
    // Tons moyens seulement : ni bruit des ombres, ni hautes lumières écrêtées.
    let mut ys: Vec<f64> = pixels.iter().map(|c| c[1]).collect();
    ys.sort_by(f64::total_cmp);
    let at = |q: f64| ys[((ys.len() - 1) as f64 * q) as usize];
    let (y_lo, y_hi) = (at(0.2), at(0.98).min(0.9 * at(1.0)));
    let mid: Vec<[f64; 3]> = pixels
        .into_iter()
        .filter(|c| c[1] >= y_lo && c[1] <= y_hi)
        .collect();
    // L'appareil se trompe rarement de beaucoup : les gris candidats sont
    // cherchés près de son neutre (le feuillage ou un ciel ne sont pas des gris).
    let near: Vec<[f64; 3]> = mid
        .iter()
        .filter(|c| uv_distance(**c, D65_WHITE) < CANDIDATE_DISTANCE)
        .copied()
        .collect();
    let mid = if near.len() >= mid.len() / 20 {
        near
    } else {
        mid
    };
    let min_count = (mid.len() / 100).max(20);
    if mid.len() < min_count {
        return WhiteBalance::AsShot;
    }
    let mean = |set: &[[f64; 3]]| {
        let mut acc = [0.0; 3];
        for c in set {
            // Chaque pixel compte pour sa couleur, pas pour sa luminosité.
            for k in 0..3 {
                acc[k] += c[k] / c[1];
            }
        }
        acc.map(|v| v / set.len() as f64)
    };

    // Monde gris sur les 70 % les moins saturés autour de la couleur moyenne.
    let overall = mean(&mid);
    let mut by_distance: Vec<(f64, [f64; 3])> =
        mid.iter().map(|c| (uv_distance(*c, overall), *c)).collect();
    by_distance.sort_by(|a, b| a.0.total_cmp(&b.0));
    let calm: Vec<[f64; 3]> = by_distance[..by_distance.len() * 7 / 10]
        .iter()
        .map(|(_, c)| *c)
        .collect();
    let mut grey = mean(&calm);

    // Affinage : les pixels que cette estimation rend presque neutres.
    let mut neutral_count = 0;
    for threshold in [0.02, 0.012, 0.008] {
        let wb = white::neutralizing(grey, image.as_shot_white);
        let m = white::correction_xyz(&wb, image.as_shot_white);
        let neutral: Vec<[f64; 3]> = mid
            .iter()
            .filter(|c| uv_distance(white::apply(&m, **c), D65_WHITE) < threshold)
            .copied()
            .collect();
        if neutral.len() < min_count {
            break;
        }
        neutral_count = neutral.len();
        grey = mean(&neutral);
    }
    // Aucun gris crédible (feuillage, ciel, mur coloré) : la balance du boîtier.
    if (neutral_count as f64) < MIN_NEUTRAL_SHARE * mid.len() as f64 {
        return as_shot(image);
    }
    match white::neutralizing(grey, image.as_shot_white) {
        // Une teinte forte n'est pas celle d'une lumière réelle mais la couleur
        // dominante du sujet (feuillage) : la balance du boîtier reste.
        WhiteBalance::Custom { tint, .. } if tint.abs() > AUTO_TINT_LIMIT => as_shot(image),
        wb => wb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::pipeline::XYZ_TO_REC2020;

    fn image_from_xyz(w: u32, h: u32, f: impl Fn(u32, u32) -> [f64; 3]) -> WorkingImage {
        let m = XYZ_TO_REC2020.map(|r| r.map(f64::from));
        let mut rgb = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgb.extend(white::apply(&m, f(x, y)).map(|v| v as f32));
            }
        }
        WorkingImage {
            width: w,
            height: h,
            rgb,
            as_shot_white: D65_WHITE,
        }
    }

    /// Une scène : dégradé de gris et quelques aplats de couleur vive.
    fn scene(gain: f64, light: [f64; 3]) -> WorkingImage {
        let colors = [[0.3, 0.2, 0.05], [0.1, 0.25, 0.08], [0.15, 0.1, 0.4]];
        image_from_xyz(300, 200, |x, y| {
            let base = if y < 50 {
                let c = colors[(x / 100) as usize];
                [c[0] / 0.25, c[1] / 0.25, c[2] / 0.25]
            } else {
                [1.0, 1.0, 1.0]
            };
            let v = gain * (0.02 + 0.9 * (x as f64 / 300.0).powi(2));
            [
                light[0] * base[0] * v,
                light[1] * base[1] * v,
                light[2] * base[2] * v,
            ]
        })
    }

    #[test]
    fn image_sous_exposee_relevee_au_gris_moyen() {
        let img = scene(0.08, D65_WHITE);
        let light = auto_light(&img, &DevelopSettings::default());
        assert!(light.exposure > 1.0, "{light:?}");
        let s = DevelopSettings {
            light,
            ..DevelopSettings::default()
        };
        let m = Luma::of(&sample(&img), &s);
        assert!(
            (m.percentile(0.5) - MEDIAN_TARGET).abs() < 12.0,
            "{}",
            m.percentile(0.5)
        );
        assert!(m.share(HIGHLIGHT_LEVEL, true) <= 0.02);
    }

    #[test]
    fn deterministe_et_garde_le_reste() {
        let img = scene(0.5, D65_WHITE);
        let mut base = DevelopSettings::default();
        base.light.brightness = 20.0;
        base.light.contrast = -50.0;
        base.color.saturation = 30.0;
        let a = auto_light(&img, &base);
        let b = auto_light(&img, &base);
        assert_eq!(a, b);
        assert_eq!(a.brightness, 20.0);
        assert!(a.contrast >= 0.0);
    }

    #[test]
    fn dominante_tungstene_retrouvee() {
        let tungsten = white::white_of(white::xy_from_temperature(3200.0, 0.0));
        let img = scene(0.5, tungsten);
        let WhiteBalance::Custom { temperature, tint } = auto_white(&img) else {
            panic!("balance attendue");
        };
        assert!((temperature - 3200.0).abs() < 250.0, "{temperature} K");
        assert!(tint.abs() < 10.0, "teinte {tint}");
        // Sans gris, la balance du boîtier (ici D65, environ 6500 K) reste.
        let colored = image_from_xyz(100, 100, |x, _| {
            let v = 0.1 + 0.4 * f64::from(x) / 100.0;
            [0.2 * v / 0.35, v, 0.1 * v / 0.35]
        });
        let WhiteBalance::Custom { temperature, tint } = auto_white(&colored) else {
            panic!("balance attendue");
        };
        assert!(
            (temperature - 6500.0).abs() < 100.0 && tint.abs() < 12.0,
            "{temperature} {tint}"
        );
        // Une scène déjà neutre reste à peu près à la balance du boîtier.
        let WhiteBalance::Custom { temperature, .. } = auto_white(&scene(0.5, D65_WHITE)) else {
            panic!("balance attendue");
        };
        assert!((temperature - 6500.0).abs() < 400.0, "{temperature} K");
    }
}
