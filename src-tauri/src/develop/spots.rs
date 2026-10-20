//! Correcteur de défauts (processus 1) : chaque correction remplace un disque
//! de l'image par un disque voisin de la même photo. En mode correction, la
//! différence de teinte et de lumière entre les deux bords est fondue dans le
//! disque (interpolation harmonique) ; en mode clonage, la source est copiée
//! telle quelle. Rien n'est inventé.
//!
//! Appliqué à l'image linéaire avant la géométrie, dans le repère de la
//! *source* (orientation du boîtier appliquée) : positions en fractions de la
//! largeur et de la hauteur, rayon en fraction du côté long, si bien qu'une
//! correction vaut pour l'aperçu comme pour la pleine définition. Les sources
//! sont toujours lues dans l'image non corrigée : l'aperçu et une région
//! zoomée donnent le même résultat.

use super::pipeline::{LUMA_REC2020, WorkingImage};
use super::settings::{Spot, SpotMode};

/// Anneau autour du disque lu pour fondre les bords, en pixels.
const RING: f64 = 2.0;
/// Côté de la grille de l'interpolation harmonique : la différence des bords
/// varie lentement, une grille grossière suffit et reste rapide.
const GRID: usize = 24;
/// Itérations de surrelaxation sur la grille.
const SOR_ITERATIONS: usize = 200;
const SOR_OMEGA: f64 = 1.85;

/// Région d'une image non corrigée, lue par [`heal`] : (left, top, width,
/// height) en pixels de l'image entière, bornée par l'appelant.
pub type Fetch<'a> = dyn Fn(u32, u32, u32, u32) -> WorkingImage + 'a;

/// Valeur du pixel (x, y) de l'image entière, lue dans la région `img`
/// (bords répétés au-delà).
fn at(img: &WorkingImage, x: i64, y: i64) -> [f32; 3] {
    let (ox, oy, _, _) = img.frame();
    let lx = (x - i64::from(ox)).clamp(0, i64::from(img.width) - 1) as usize;
    let ly = (y - i64::from(oy)).clamp(0, i64::from(img.height) - 1) as usize;
    let i = (ly * img.width as usize + lx) * 3;
    [img.rgb[i], img.rgb[i + 1], img.rgb[i + 2]]
}

/// Rectangle (x0, y0, x1, y1) borné à l'image entière ; `None` s'il est vide.
fn clip(x0: f64, y0: f64, x1: f64, y1: f64, fw: u32, fh: u32) -> Option<(u32, u32, u32, u32)> {
    let x0 = x0.floor().clamp(0.0, f64::from(fw)) as u32;
    let y0 = y0.floor().clamp(0.0, f64::from(fh)) as u32;
    let x1 = x1.ceil().clamp(0.0, f64::from(fw)) as u32;
    let y1 = y1.ceil().clamp(0.0, f64::from(fh)) as u32;
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

/// Une correction en pixels de l'image entière (fw × fh).
struct Disk {
    cx: f64,
    cy: f64,
    r: f64,
    dx: i64,
    dy: i64,
}

impl Disk {
    fn of(spot: &Spot, fw: u32, fh: u32) -> Self {
        let (w, h) = (f64::from(fw), f64::from(fh));
        let r = (spot.radius * w.max(h)).max(1.0);
        let (cx, cy) = (spot.x * w, spot.y * h);
        Self {
            cx,
            cy,
            r,
            dx: (spot.source_x * w - cx).round() as i64,
            dy: (spot.source_y * h - cy).round() as i64,
        }
    }
}

/// Différence (destination − source) des bords, prolongée dans le disque par
/// interpolation harmonique, sur une grille de `GRID` × `GRID` couvrant
/// (x0, y0)–(x1, y1).
fn membrane(
    disk: &Disk,
    dst: &WorkingImage,
    src: &WorkingImage,
    bounds: [f64; 4],
) -> Vec<[f64; 3]> {
    let [x0, y0, x1, y1] = bounds;
    let (cw, ch) = ((x1 - x0) / GRID as f64, (y1 - y0) / GRID as f64);
    let mut grid = vec![[0.0f64; 3]; GRID * GRID];
    let mut fixed = vec![false; GRID * GRID];
    let (mut mean, mut n) = ([0.0f64; 3], 0.0);
    for j in 0..GRID {
        for i in 0..GRID {
            let (gx, gy) = (x0 + (i as f64 + 0.5) * cw, y0 + (j as f64 + 0.5) * ch);
            if (gx - disk.cx).hypot(gy - disk.cy) < disk.r {
                continue;
            }
            // Moyenne de la différence sur la cellule : le grain n'y passe pas.
            let (mut acc, mut k) = ([0.0f64; 3], 0.0);
            let (px0, py0) = (
                (gx - cw / 2.0).floor() as i64,
                (gy - ch / 2.0).floor() as i64,
            );
            let (px1, py1) = (
                ((gx + cw / 2.0).ceil() as i64).max(px0 + 1),
                ((gy + ch / 2.0).ceil() as i64).max(py0 + 1),
            );
            for y in py0..py1 {
                for x in px0..px1 {
                    let (d, s) = (at(dst, x, y), at(src, x + disk.dx, y + disk.dy));
                    for c in 0..3 {
                        acc[c] += f64::from(d[c] - s[c]);
                    }
                    k += 1.0;
                }
            }
            let v = acc.map(|a| a / k);
            grid[j * GRID + i] = v;
            fixed[j * GRID + i] = true;
            for c in 0..3 {
                mean[c] += v[c];
            }
            n += 1.0;
        }
    }
    if n > 0.0 {
        let mean = mean.map(|m| m / n);
        for (v, f) in grid.iter_mut().zip(&fixed) {
            if !f {
                *v = mean;
            }
        }
    }
    for _ in 0..SOR_ITERATIONS {
        for j in 1..GRID - 1 {
            for i in 1..GRID - 1 {
                let k = j * GRID + i;
                if fixed[k] {
                    continue;
                }
                let avg: [f64; 3] = std::array::from_fn(|c| {
                    (grid[k - 1][c] + grid[k + 1][c] + grid[k - GRID][c] + grid[k + GRID][c]) / 4.0
                });
                for (v, a) in grid[k].iter_mut().zip(avg) {
                    *v += SOR_OMEGA * (a - *v);
                }
            }
        }
    }
    grid
}

/// Valeur bilinéaire de la grille au point (x, y) de l'image entière.
fn grid_at(grid: &[[f64; 3]], bounds: [f64; 4], x: f64, y: f64) -> [f64; 3] {
    let [x0, y0, x1, y1] = bounds;
    let gx = ((x - x0) / (x1 - x0) * GRID as f64 - 0.5).clamp(0.0, (GRID - 1) as f64);
    let gy = ((y - y0) / (y1 - y0) * GRID as f64 - 0.5).clamp(0.0, (GRID - 1) as f64);
    let (i0, j0) = (gx.floor() as usize, gy.floor() as usize);
    let (i1, j1) = ((i0 + 1).min(GRID - 1), (j0 + 1).min(GRID - 1));
    let (fx, fy) = (gx - i0 as f64, gy - j0 as f64);
    std::array::from_fn(|c| {
        let top = grid[j0 * GRID + i0][c] * (1.0 - fx) + grid[j0 * GRID + i1][c] * fx;
        let bottom = grid[j1 * GRID + i0][c] * (1.0 - fx) + grid[j1 * GRID + i1][c] * fx;
        top * (1.0 - fy) + bottom * fy
    })
}

/// Applique les corrections à `img` (l'image entière, ou une région de
/// celle-ci, voir [`WorkingImage::frame`]). `fetch` lit l'image non corrigée.
pub fn heal(img: &mut WorkingImage, spots: &[Spot], fetch: &Fetch) {
    let (ox, oy, fw, fh) = img.frame();
    let (rx1, ry1) = (ox + img.width, oy + img.height);
    for spot in spots {
        let disk = Disk::of(spot, fw, fh);
        let reach = disk.r + RING;
        let bounds = [
            disk.cx - reach,
            disk.cy - reach,
            disk.cx + reach,
            disk.cy + reach,
        ];
        // Pixels touchés, dans la région.
        let Some((x0, y0, x1, y1)) = clip(
            disk.cx - disk.r,
            disk.cy - disk.r,
            disk.cx + disk.r,
            disk.cy + disk.r,
            fw,
            fh,
        ) else {
            continue;
        };
        let (x0, y0, x1, y1) = (x0.max(ox), y0.max(oy), x1.min(rx1), y1.min(ry1));
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        let read = |b: [f64; 4], dx: i64, dy: i64| {
            let (dx, dy) = (dx as f64, dy as f64);
            clip(b[0] + dx, b[1] + dy, b[2] + dx, b[3] + dy, fw, fh)
                .map(|(a, b, c, d)| fetch(a, b, c - a, d - b))
        };
        let Some(src) = read(bounds, disk.dx, disk.dy) else {
            continue;
        };
        let grid = match spot.mode {
            SpotMode::Heal => match read(bounds, 0, 0) {
                Some(dst) => Some(membrane(&disk, &dst, &src, bounds)),
                None => continue,
            },
            SpotMode::Clone => None,
        };
        // Adoucissement : le disque se fond dans l'image sur sa couronne.
        let inner = disk.r * (1.0 - spot.feather / 100.0);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let d = (px - disk.cx).hypot(py - disk.cy);
                if d >= disk.r {
                    continue;
                }
                let alpha = if d <= inner {
                    1.0
                } else {
                    let t = (d - inner) / (disk.r - inner);
                    1.0 - t * t * (3.0 - 2.0 * t)
                } as f32;
                let s = at(&src, i64::from(x) + disk.dx, i64::from(y) + disk.dy);
                let fill: [f32; 3] = match &grid {
                    Some(g) => {
                        let delta = grid_at(g, bounds, px, py);
                        std::array::from_fn(|c| (f64::from(s[c]) + delta[c]).max(0.0) as f32)
                    }
                    None => s,
                };
                let i = (((y - oy) * img.width + (x - ox)) * 3) as usize;
                for (v, f) in img.rgb[i..i + 3].iter_mut().zip(fill) {
                    *v += (f - *v) * alpha;
                }
            }
        }
    }
}

/// L'image entière corrigée (aperçu) ; les sources sont lues dans `src`.
pub fn healed(src: &WorkingImage, spots: &[Spot]) -> WorkingImage {
    let mut out = src.clone();
    if !spots.is_empty() {
        heal(&mut out, spots, &|l, t, w, h| src.sub(l, t, w, h));
    }
    out
}

/// Luminance perceptuelle (racine de la luminance linéaire).
fn lightness(img: &WorkingImage, x: i64, y: i64) -> f64 {
    let p = at(img, x, y);
    f64::from(p[0] * LUMA_REC2020[0] + p[1] * LUMA_REC2020[1] + p[2] * LUMA_REC2020[2])
        .max(0.0)
        .sqrt()
}

/// Points d'un disque (ou d'un anneau), au pas `step`, relatifs au centre.
fn offsets(r_in: f64, r_out: f64, step: f64) -> Vec<(f64, f64)> {
    let n = (r_out / step).ceil() as i64;
    let mut out = Vec::new();
    for j in -n..=n {
        for i in -n..=n {
            let (x, y) = (i as f64 * step, j as f64 * step);
            let d = x.hypot(y);
            if d >= r_in && d <= r_out {
                out.push((x, y));
            }
        }
    }
    out
}

/// Source choisie automatiquement pour une correction en (x, y) de rayon
/// `radius` (fractions, comme [`Spot`]) : le disque voisin dont l'anneau
/// ressemble le plus à celui de la destination et dont l'intérieur a la même
/// texture, sans empiéter sur la destination ni sur une autre correction.
pub fn find_source(img: &WorkingImage, x: f64, y: f64, radius: f64, others: &[Spot]) -> (f64, f64) {
    let (w, h) = (f64::from(img.width), f64::from(img.height));
    let long = w.max(h);
    let r = (radius * long).max(1.0);
    let (cx, cy) = (x * w, y * h);
    let step = (r / 6.0).max(1.0);
    let ring = offsets(r, r * 1.5 + 2.0, step);
    let inside = offsets(0.0, r, step);
    let sample = |px: f64, py: f64, pts: &[(f64, f64)]| -> Vec<f64> {
        pts.iter()
            .map(|(dx, dy)| lightness(img, (px + dx).floor() as i64, (py + dy).floor() as i64))
            .collect()
    };
    let spread = |v: &[f64]| {
        let m = v.iter().sum::<f64>() / v.len().max(1) as f64;
        (v.iter().map(|a| (a - m) * (a - m)).sum::<f64>() / v.len().max(1) as f64).sqrt()
    };
    let target = sample(cx, cy, &ring);
    let texture = spread(&target);
    let reach = r * 1.5 + 2.0;
    let blocked = |sx: f64, sy: f64| {
        others.iter().any(|o| {
            let (ox, oy, or) = (o.x * w, o.y * h, o.radius * long);
            (sx - ox).hypot(sy - oy) < r + or
        })
    };
    let mut best: Option<(f64, f64, f64)> = None;
    for k in [2.2, 3.0, 4.0, 5.5] {
        let dist = (k * r).max(2.0 * r + 3.0);
        for a in 0..24 {
            let angle = f64::from(a) * std::f64::consts::TAU / 24.0;
            let (sx, sy) = (cx + dist * angle.cos(), cy + dist * angle.sin());
            if sx - reach < 0.0 || sy - reach < 0.0 || sx + reach > w || sy + reach > h {
                continue;
            }
            if blocked(sx, sy) {
                continue;
            }
            let around = sample(sx, sy, &ring);
            let ring_diff = target
                .iter()
                .zip(&around)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f64>()
                / target.len().max(1) as f64;
            let tex = spread(&sample(sx, sy, &inside)) - texture;
            // À ressemblance égale, la source la plus proche.
            let score = (ring_diff + tex * tex) * (1.0 + 0.05 * k);
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, sx, sy));
            }
        }
    }
    match best {
        Some((_, sx, sy)) => (sx / w, sy / h),
        // Photo trop petite pour ce rayon : la source la plus proche dans l'image.
        None => {
            let sx = if cx + 2.2 * r <= w {
                cx + 2.2 * r
            } else {
                cx - 2.2 * r
            };
            ((sx / w).clamp(0.0, 1.0), y)
        }
    }
}

/// Visualisation des poussières : luminance moins sa moyenne locale, très
/// accentuée, en gris. `long` est le côté long de l'image entière, pour que
/// la portée du flou suive la définition. `rgba` est modifié sur place.
pub fn paint_dust(rgba: &mut [u8], width: u32, height: u32, long: u32) {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 {
        return;
    }
    let radius = (long / 300).max(4) as usize;
    let luma: Vec<f32> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| 0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]))
        .collect();
    // Moyenne locale par une table de sommes cumulées.
    let mut sum = vec![0.0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0.0f64;
        for x in 0..w {
            row += f64::from(luma[y * w + x]);
            sum[(y + 1) * (w + 1) + x + 1] = sum[y * (w + 1) + x + 1] + row;
        }
    }
    for y in 0..h {
        let (ya, yb) = (y.saturating_sub(radius), (y + radius + 1).min(h));
        for x in 0..w {
            let (xa, xb) = (x.saturating_sub(radius), (x + radius + 1).min(w));
            let total = sum[yb * (w + 1) + xb] - sum[ya * (w + 1) + xb] - sum[yb * (w + 1) + xa]
                + sum[ya * (w + 1) + xa];
            let mean = total / ((xb - xa) * (yb - ya)) as f64;
            let v = (128.0 + 6.0 * (f64::from(luma[y * w + x]) - mean)).clamp(0.0, 255.0) as u8;
            let i = (y * w + x) * 4;
            rgba[i..i + 3].fill(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dégradé doux avec une texture régulière, et une tache sombre.
    fn image_with_dust(w: u32, h: u32, dust: (f64, f64, f64)) -> WorkingImage {
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let base = 0.2 + 0.3 * f64::from(x) / f64::from(w);
                let texture = 0.01 * ((f64::from(x) * 0.7).sin() + (f64::from(y) * 0.9).cos());
                let d = (f64::from(x) + 0.5 - dust.0).hypot(f64::from(y) + 0.5 - dust.1);
                let shade = if d < dust.2 { 0.4 } else { 1.0 };
                let v = ((base + texture) * shade) as f32;
                rgb.extend_from_slice(&[v, v * 0.9, v * 0.8]);
            }
        }
        WorkingImage {
            width: w,
            height: h,
            rgb,
            as_shot_white: [0.95, 1.0, 1.09],
            scale: 1.0,
            crop: None,
        }
    }

    fn spot(x: f64, y: f64, radius: f64, sx: f64, sy: f64, mode: SpotMode) -> Spot {
        Spot {
            x,
            y,
            radius,
            source_x: sx,
            source_y: sy,
            feather: 0.0,
            mode,
        }
    }

    fn mean_in(img: &WorkingImage, cx: f64, cy: f64, r: f64) -> f64 {
        let (mut acc, mut n) = (0.0, 0.0);
        for y in 0..img.height {
            for x in 0..img.width {
                if (f64::from(x) + 0.5 - cx).hypot(f64::from(y) + 0.5 - cy) < r {
                    acc += f64::from(img.rgb[((y * img.width + x) * 3) as usize]);
                    n += 1.0;
                }
            }
        }
        acc / n
    }

    #[test]
    fn la_correction_efface_la_tache_et_raccorde_les_bords() {
        let (w, h) = (200, 120);
        let img = image_with_dust(w, h, (100.0, 60.0, 6.0));
        let clean = image_with_dust(w, h, (0.0, 0.0, 0.0));
        let expected = mean_in(&clean, 100.0, 60.0, 6.0);
        let before = mean_in(&img, 100.0, 60.0, 6.0);
        assert!(before < expected * 0.6);
        // Source à gauche, plus sombre (dégradé) : la correction compense.
        let s = spot(0.5, 0.5, 9.0 / 200.0, 0.4, 0.5, SpotMode::Heal);
        let healed = healed(&img, std::slice::from_ref(&s));
        let after = mean_in(&healed, 100.0, 60.0, 6.0);
        assert!(
            (after - expected).abs() < 0.01,
            "{after} au lieu de {expected}"
        );
        // Le clonage copie la source telle quelle : plus sombre, du dégradé.
        let cloned = healed_with(
            &img,
            Spot {
                mode: SpotMode::Clone,
                ..s
            },
        );
        let copied = mean_in(&cloned, 100.0, 60.0, 6.0);
        assert!(copied < expected - 0.02, "{copied}");
        // Hors du disque, rien ne change.
        assert_eq!(healed.rgb[..300], img.rgb[..300]);
    }

    fn healed_with(img: &WorkingImage, s: Spot) -> WorkingImage {
        healed(img, &[s])
    }

    #[test]
    fn une_region_donne_le_meme_resultat_que_l_image_entiere() {
        let img = image_with_dust(160, 100, (60.0, 50.0, 5.0));
        let spots = [spot(0.375, 0.5, 0.05, 0.6, 0.5, SpotMode::Heal)];
        let whole = healed(&img, &spots);
        // La région ne contient pas la source.
        let (l, t, rw, rh) = (40, 30, 40, 40);
        let mut region = img.sub(l, t, rw, rh);
        heal(&mut region, &spots, &|a, b, c, d| img.sub(a, b, c, d));
        for y in 0..rh {
            for x in 0..rw {
                let i = ((y * rw + x) * 3) as usize;
                let j = (((y + t) * 160 + x + l) * 3) as usize;
                assert!((region.rgb[i] - whole.rgb[j]).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn la_source_automatique_evite_la_destination_et_les_autres_taches() {
        let img = image_with_dust(300, 200, (150.0, 100.0, 6.0));
        let (sx, sy) = find_source(&img, 0.5, 0.5, 8.0 / 300.0, &[]);
        let d = (sx * 300.0 - 150.0).hypot(sy * 200.0 - 100.0);
        assert!(d >= 16.0, "source à {d} px");
        assert!(sx > 0.0 && sx < 1.0 && sy > 0.0 && sy < 1.0);
        // Même texture verticale (dégradé horizontal) : une source proche en
        // hauteur, à peu près à la même abscisse.
        assert!((sx * 300.0 - 150.0).abs() < 30.0, "{sx}");
        // Déterministe.
        assert_eq!(find_source(&img, 0.5, 0.5, 8.0 / 300.0, &[]), (sx, sy));
        // Une autre correction à cet endroit l'écarte.
        let other = spot(sx, sy, 8.0 / 300.0, 0.1, 0.1, SpotMode::Heal);
        let (ax, ay) = find_source(&img, 0.5, 0.5, 8.0 / 300.0, &[other]);
        assert!((ax * 300.0 - sx * 300.0).hypot(ay * 200.0 - sy * 200.0) >= 16.0);
    }

    #[test]
    fn la_visualisation_fait_ressortir_une_tache() {
        let (w, h) = (100u32, 80u32);
        let mut rgba = vec![200u8; (w * h * 4) as usize];
        for y in 38..42 {
            for x in 48..52 {
                let i = ((y * w + x) * 4) as usize;
                rgba[i..i + 3].fill(180);
            }
        }
        paint_dust(&mut rgba, w, h, 1200);
        let px = |x: u32, y: u32| rgba[((y * w + x) * 4) as usize];
        assert!(px(10, 10).abs_diff(128) <= 2);
        assert!(px(50, 40) < 60, "{}", px(50, 40));
    }
}
