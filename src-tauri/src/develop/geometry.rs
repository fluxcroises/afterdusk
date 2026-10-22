//! Géométrie (processus 1) : quarts de tour et miroirs, redressement autour
//! du centre, puis recadrage. Rééchantillonnage bilinéaire, une fois par
//! géométrie, avant le reste du pipeline : réduction du bruit, netteté et GPU
//! travaillent sur l'image déjà transformée, et le vignetage suit le cadre
//! recadré.
//!
//! Repères : la *source* est l'image développée (orientation du boîtier
//! appliquée) ; le *canevas* est la source tournée par quarts de tour et
//! retournée, puis redressée (même taille) ; la *sortie* est le recadrage du
//! canevas. Coordonnées continues en pixels, centres de pixel à +0,5.

use super::pipeline::{Crop, LUMA_REC2020, WorkingImage};
use super::settings::{CropRect, Geometry};

/// Taille du canevas orienté d'une source `sw` × `sh`.
pub fn oriented(sw: f64, sh: f64, quarter: u8) -> (f64, f64) {
    if quarter % 2 == 1 { (sh, sw) } else { (sw, sh) }
}

/// Point du canevas vers le point de la source qu'il montre.
pub fn canvas_to_source(p: [f64; 2], g: &Geometry, sw: f64, sh: f64) -> [f64; 2] {
    let (w, h) = oriented(sw, sh, g.quarter);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let (s, c) = g.angle.to_radians().sin_cos();
    let (dx, dy) = (p[0] - cx, p[1] - cy);
    // Le contenu tourne de `angle` dans le sens horaire : on défait la rotation.
    let mut x = cx + dx * c + dy * s;
    let mut y = cy - dx * s + dy * c;
    if g.flip_h {
        x = w - x;
    }
    if g.flip_v {
        y = h - y;
    }
    match g.quarter % 4 {
        0 => [x, y],
        1 => [y, sh - x],
        2 => [sw - x, sh - y],
        _ => [sw - y, x],
    }
}

/// Dimensions et origine du recadrage dans le canevas d'une source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    /// Coin haut gauche du recadrage, en pixels du canevas.
    pub left: f64,
    pub top: f64,
    /// Taille de la sortie, en pixels.
    pub width: u32,
    pub height: u32,
}

pub fn plan(g: &Geometry, sw: f64, sh: f64) -> Plan {
    let (w, h) = oriented(sw, sh, g.quarter);
    let c = &g.crop;
    Plan {
        left: c.left * w,
        top: c.top * h,
        width: (((c.right - c.left) * w).round() as u32).max(1),
        height: (((c.bottom - c.top) * h).round() as u32).max(1),
    }
}

/// Valeur bilinéaire de `src` (région d'une source plus grande) au point
/// `[x, y]` de la source entière ; noir hors de la source.
fn sample(src: &WorkingImage, x: f64, y: f64) -> [f32; 3] {
    let (ox, oy, fw, fh) = src.frame();
    if x < 0.0 || y < 0.0 || x > f64::from(fw) || y > f64::from(fh) {
        return [0.0; 3];
    }
    // Centres de pixel à +0,5 ; bords de la région : valeur du bord.
    let lx = (x - f64::from(ox) - 0.5).clamp(0.0, f64::from(src.width - 1));
    let ly = (y - f64::from(oy) - 0.5).clamp(0.0, f64::from(src.height - 1));
    let (x0, y0) = (lx.floor() as u32, ly.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(src.width - 1), (y0 + 1).min(src.height - 1));
    let (fx, fy) = ((lx - f64::from(x0)) as f32, (ly - f64::from(y0)) as f32);
    let at = |xx: u32, yy: u32, c: usize| src.rgb[((yy * src.width + xx) * 3) as usize + c];
    std::array::from_fn(|c| {
        let top = at(x0, y0, c) * (1.0 - fx) + at(x1, y0, c) * fx;
        let bottom = at(x0, y1, c) * (1.0 - fx) + at(x1, y1, c) * fx;
        top * (1.0 - fy) + bottom * fy
    })
}

/// La région (left, top, width, height) de la sortie, rééchantillonnée depuis
/// `src` (une région de la source entière, voir [`WorkingImage::frame`]).
pub fn transform_region(
    src: &WorkingImage,
    g: &Geometry,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> WorkingImage {
    let (_, _, fw, fh) = src.frame();
    let (sw, sh) = (f64::from(fw), f64::from(fh));
    let p = plan(g, sw, sh);
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for v in 0..height {
        for u in 0..width {
            let canvas = [
                p.left + f64::from(left + u) + 0.5,
                p.top + f64::from(top + v) + 0.5,
            ];
            let s = canvas_to_source(canvas, g, sw, sh);
            rgb.extend_from_slice(&sample(src, s[0], s[1]));
        }
    }
    let whole = left == 0 && top == 0 && width == p.width && height == p.height;
    WorkingImage {
        width,
        height,
        rgb,
        as_shot_white: src.as_shot_white,
        rendered: src.rendered,
        scale: src.scale,
        crop: (!whole).then_some(Crop {
            left,
            top,
            full_width: p.width,
            full_height: p.height,
        }),
    }
}

/// L'image entière transformée (aperçu).
pub fn transform(src: &WorkingImage, g: &Geometry) -> WorkingImage {
    if g.is_identity() {
        return src.clone();
    }
    let (_, _, fw, fh) = src.frame();
    let p = plan(g, f64::from(fw), f64::from(fh));
    transform_region(src, g, 0, 0, p.width, p.height)
}

/// Rectangle de la source (x0, y0, x1, y1, bornés) à décoder pour rendre la
/// région (left, top, width, height) de la sortie, avec une marge de deux
/// pixels pour l'interpolation.
pub fn source_bounds(
    g: &Geometry,
    sw: u32,
    sh: u32,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> (u32, u32, u32, u32) {
    let (fw, fh) = (f64::from(sw), f64::from(sh));
    let p = plan(g, fw, fh);
    let corners = [
        (left, top),
        (left + width, top),
        (left, top + height),
        (left + width, top + height),
    ]
    .map(|(u, v)| canvas_to_source([p.left + f64::from(u), p.top + f64::from(v)], g, fw, fh));
    let min = |k: usize| corners.iter().map(|c| c[k]).fold(f64::INFINITY, f64::min);
    let max = |k: usize| {
        corners
            .iter()
            .map(|c| c[k])
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let clamp = |v: f64, hi: u32| (v.max(0.0) as u32).min(hi);
    (
        clamp(min(0) - 2.0, sw.saturating_sub(1)),
        clamp(min(1) - 2.0, sh.saturating_sub(1)),
        clamp(max(0).ceil() + 2.0, sw),
        clamp(max(1).ceil() + 2.0, sh),
    )
}

/// Le recadrage ramené dans l'image redressée : sans coin vide. Le centre
/// rejoint l'image s'il en sort, puis le cadre rétrécit autour de lui en
/// gardant ses proportions.
pub fn constrain(g: &Geometry, sw: f64, sh: f64) -> CropRect {
    let (w, h) = oriented(sw, sh, g.quarter);
    let c = g.crop;
    let inside = |x: f64, y: f64| {
        let s = canvas_to_source([x * w, y * h], g, sw, sh);
        s[0] >= -1e-6 && s[1] >= -1e-6 && s[0] <= sw + 1e-6 && s[1] <= sh + 1e-6
    };
    let (mut cx, mut cy) = ((c.left + c.right) / 2.0, (c.top + c.bottom) / 2.0);
    for _ in 0..40 {
        if inside(cx, cy) {
            break;
        }
        cx = (cx + 0.5) / 2.0;
        cy = (cy + 0.5) / 2.0;
    }
    let (hw, hh) = ((c.right - c.left) / 2.0, (c.bottom - c.top) / 2.0);
    let fits = |k: f64| {
        [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .iter()
            .all(|(sx, sy)| inside(cx + sx * hw * k, cy + sy * hh * k))
    };
    let k = if fits(1.0) {
        1.0
    } else {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..40 {
            let mid = (lo + hi) / 2.0;
            if fits(mid) { lo = mid } else { hi = mid }
        }
        lo
    };
    CropRect {
        left: (cx - hw * k).max(0.0),
        top: (cy - hh * k).max(0.0),
        right: (cx + hw * k).min(1.0),
        bottom: (cy + hh * k).min(1.0),
    }
}

/// Plus grand redressement proposé par l'horizon automatique, en degrés.
pub const AUTO_MAX_ANGLE: f64 = 10.0;

/// Pic minimal de l'histogramme des directions, rapporté à sa moyenne. Mesuré
/// sur les six échantillons inclinés de +3° et -5° : de 1,18 à 4,3 avec de
/// vraies lignes (bâti, mobilier), 1,03 à 1,05 pour du feuillage ou des
/// branches.
pub const AUTO_MIN_PEAK: f64 = 1.15;

/// Horizon automatique : angle qui remet d'aplomb les lignes dominantes
/// proches de l'horizontale ou de la verticale (gradients pondérés par leur
/// force). L'image est celle du canevas avant redressement (quarts de tour et
/// miroirs appliqués). 0 sans ligne franche.
pub fn auto_straighten(image: &WorkingImage) -> f64 {
    let stride = (image.width.max(image.height) / 1000).max(1);
    let (w, h) = (image.width / stride, image.height / stride);
    if w < 8 || h < 8 {
        return 0.0;
    }
    let lum: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) * stride, (i / w) * stride);
            let k = ((y * image.width + x) * 3) as usize;
            let y = LUMA_REC2020[0] * image.rgb[k]
                + LUMA_REC2020[1] * image.rgb[k + 1]
                + LUMA_REC2020[2] * image.rgb[k + 2];
            y.max(0.0).powf(1.0 / 2.2)
        })
        .collect();
    const BIN: f64 = 0.25;
    let bins = (2.0 * AUTO_MAX_ANGLE / BIN) as usize + 1;
    let mut hist = vec![0.0f64; bins];
    let mut total = 0.0;
    let at = |x: u32, y: u32| f64::from(lum[(y * w + x) as usize]);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let gx = at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x - 1, y)
                - at(x - 1, y + 1);
            let gy = at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x, y - 1)
                - at(x + 1, y - 1);
            let m = gx.hypot(gy);
            if m < 0.05 {
                continue;
            }
            total += m * m;
            // Direction de la ligne (perpendiculaire au gradient), rapportée à
            // l'axe le plus proche ; pondérée par le carré de la force, pour
            // que les contours francs l'emportent sur les textures.
            let line = gy.atan2(gx).to_degrees() + 90.0;
            let a = (line + 45.0).rem_euclid(90.0) - 45.0;
            if a.abs() <= AUTO_MAX_ANGLE {
                let b = ((a + AUTO_MAX_ANGLE) / BIN).round() as usize;
                hist[b.min(bins - 1)] += m * m;
            }
        }
    }
    if total <= 0.0 {
        return 0.0;
    }
    // Lissage, puis pic affiné par une parabole.
    let smooth: Vec<f64> = (0..bins)
        .map(|i| {
            [(-2isize, 1.0), (-1, 2.0), (0, 3.0), (1, 2.0), (2, 1.0)]
                .iter()
                .map(|(d, wt)| {
                    let j = i as isize + d;
                    if j >= 0 && (j as usize) < bins {
                        hist[j as usize] * wt
                    } else {
                        0.0
                    }
                })
                .sum()
        })
        .collect();
    let (peak, &best) = smooth
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap_or((bins / 2, &0.0));
    // Pas de ligne franche : un pic à moins de 15 % au-dessus de la moyenne
    // vient du hasard des textures (feuillage, tissus), pas d'un horizon.
    let mean = smooth.iter().sum::<f64>() / bins as f64;
    if best < AUTO_MIN_PEAK * mean {
        return 0.0;
    }
    let offset = if peak > 0 && peak + 1 < bins {
        let (l, c, r) = (smooth[peak - 1], smooth[peak], smooth[peak + 1]);
        let d = l - 2.0 * c + r;
        if d.abs() > 1e-12 {
            0.5 * (l - r) / d
        } else {
            0.0
        }
    } else {
        0.0
    };
    let tilt = (peak as f64 + offset) * BIN - AUTO_MAX_ANGLE;
    // Une ligne qui descend vers la droite (sens horaire) se redresse en
    // tournant dans l'autre sens.
    ((-tilt) * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::D65_WHITE;

    fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> f32) -> WorkingImage {
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let v = f(x, y);
                rgb.extend_from_slice(&[v, v, v]);
            }
        }
        WorkingImage {
            width: w,
            height: h,
            rgb,
            as_shot_white: D65_WHITE,
            scale: 1.0,
            crop: None,
            rendered: false,
        }
    }

    fn px(img: &WorkingImage, x: u32, y: u32) -> f32 {
        img.rgb[((y * img.width + x) * 3) as usize]
    }

    #[test]
    fn quarts_de_tour_et_miroirs() {
        // Un point marqué en haut à gauche de la source 4 × 2.
        let src = image(4, 2, |x, y| if (x, y) == (0, 0) { 1.0 } else { 0.0 });
        let g = |quarter, flip_h, flip_v| Geometry {
            quarter,
            flip_h,
            flip_v,
            ..Default::default()
        };
        let r = transform(&src, &g(1, false, false));
        assert_eq!((r.width, r.height), (2, 4));
        // Quart de tour horaire : le coin haut gauche passe en haut à droite.
        assert_eq!(px(&r, 1, 0), 1.0);
        let r = transform(&src, &g(2, false, false));
        assert_eq!(px(&r, 3, 1), 1.0);
        let r = transform(&src, &g(3, false, false));
        assert_eq!(px(&r, 0, 3), 1.0);
        let r = transform(&src, &g(0, true, false));
        assert_eq!(px(&r, 3, 0), 1.0);
        let r = transform(&src, &g(0, false, true));
        assert_eq!(px(&r, 0, 1), 1.0);
        // Rien : copie exacte.
        assert_eq!(transform(&src, &Geometry::default()).rgb, src.rgb);
    }

    #[test]
    fn redressement_dans_le_sens_horaire_et_recadrage() {
        // Une ligne horizontale claire au milieu d'une source 201 × 121.
        let src = image(201, 121, |_, y| if y == 60 { 1.0 } else { 0.0 });
        let g = Geometry {
            angle: 10.0,
            ..Default::default()
        };
        let r = transform(&src, &g);
        // À 40 px à droite du centre, la ligne est descendue de 40 × tan 10°.
        let dy = (40.0f64 * 10f64.to_radians().tan()).round() as u32;
        assert!(px(&r, 100 + 40, 60 + dy) > 0.3, "{}", px(&r, 140, 60 + dy));
        assert!(px(&r, 100 + 40, 60) < 0.1);
        // Recadrage : la sortie a la taille du cadre et part de son coin.
        let crop = Geometry {
            crop: CropRect {
                left: 0.25,
                top: 0.0,
                right: 0.75,
                bottom: 1.0,
            },
            ..Default::default()
        };
        let r = transform(&src, &crop);
        assert_eq!((r.width, r.height), (101, 121));
        assert!(px(&r, 10, 60) > 0.9);
    }

    #[test]
    fn region_de_la_sortie_identique_a_la_sortie_entiere() {
        let src = image(120, 80, |x, y| ((x * 7 + y * 13) % 50) as f32 / 50.0);
        let g = Geometry {
            angle: -7.5,
            quarter: 1,
            flip_h: true,
            crop: CropRect {
                left: 0.1,
                top: 0.15,
                right: 0.85,
                bottom: 0.9,
            },
            ..Default::default()
        };
        let whole = transform(&src, &g);
        let part = transform_region(&src, &g, 12, 20, 15, 10);
        assert_eq!(part.crop.map(|c| (c.left, c.top)), Some((12, 20)));
        for v in 0..10 {
            for u in 0..15 {
                assert_eq!(px(&part, u, v), px(&whole, 12 + u, 20 + v), "({u}, {v})");
            }
        }
        // Les bornes de la source couvrent bien la région.
        let (x0, y0, x1, y1) = source_bounds(&g, 120, 80, 12, 20, 15, 10);
        assert!(x0 < x1 && y0 < y1 && x1 <= 120 && y1 <= 80);
    }

    #[test]
    fn recadrage_sans_coin_vide() {
        let g = Geometry {
            angle: 12.0,
            ..Default::default()
        };
        let c = constrain(&g, 600.0, 400.0);
        assert!(c.right - c.left < 1.0 && c.bottom - c.top < 1.0, "{c:?}");
        // Proportions gardées, centré.
        let ratio = (c.right - c.left) / (c.bottom - c.top);
        assert!((ratio - 1.0).abs() < 1e-6);
        assert!(((c.left + c.right) / 2.0 - 0.5).abs() < 1e-9);
        // Les coins du cadre sont dans l'image.
        let g2 = Geometry { crop: c, ..g };
        let r = transform(&image(600, 400, |_, _| 1.0), &g2);
        let corners = [
            (0, 0),
            (r.width - 1, 0),
            (0, r.height - 1),
            (r.width - 1, r.height - 1),
        ];
        assert!(corners.iter().all(|&(x, y)| px(&r, x, y) > 0.99));
        // Sans redressement, rien ne change.
        assert!(constrain(&Geometry::default(), 600.0, 400.0).is_full());
    }

    #[test]
    fn horizon_automatique() {
        // Bandes douces inclinées de 4° dans le sens horaire : redressement de -4°.
        let tilt = 4f64.to_radians().tan();
        let src = image(800, 500, |x, y| {
            let yy = f64::from(y) - f64::from(x) * tilt;
            (0.3 + 0.25 * (yy * std::f64::consts::TAU / 60.0).sin()) as f32
        });
        let a = auto_straighten(&src);
        assert!((a + 4.0).abs() < 0.3, "{a}");
        // Une image unie n'a pas d'horizon.
        assert_eq!(auto_straighten(&image(300, 200, |_, _| 0.4)), 0.0);
    }
}
