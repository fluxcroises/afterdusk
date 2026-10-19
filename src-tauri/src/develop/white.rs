//! Balance des blancs (processus 1) : température et teinte, préréglages et
//! pipette.
//!
//! LibRaw applique la balance du boîtier : le blanc de la prise de vue devient
//! le blanc D65 de l'image. Changer de balance revient à adapter l'image (en
//! XYZ, par la transformation de Bradford) comme si la lumière avait été celle
//! choisie : une seule matrice 3 × 3, calculée une fois par rendu.
//!
//! Température en kelvins sur le lieu de Planck ; teinte en unités de type
//! Lightroom : `teinte = -Duv × 3000` (positif = magenta, sous le lieu).

use super::settings::WhiteBalance;
use crate::raw::D65_WHITE;

pub type Matrix = [[f64; 3]; 3];

/// Bornes des réglages (décision 0005).
pub const TEMPERATURE_RANGE: (f64, f64) = (2000.0, 50_000.0);
pub const TINT_RANGE: (f64, f64) = (-150.0, 150.0);
const TINT_SCALE: f64 = 3000.0;

/// Préréglages, aux valeurs usuelles des logiciels de développement.
pub const PRESETS: [(&str, f64, f64); 6] = [
    ("daylight", 5500.0, 10.0),
    ("cloudy", 6500.0, 10.0),
    ("shade", 7500.0, 10.0),
    ("tungsten", 2850.0, 0.0),
    ("fluorescent", 3800.0, 21.0),
    ("flash", 5500.0, 0.0),
];

const BRADFORD: Matrix = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

pub fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
}

pub fn apply(m: &Matrix, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| (0..3).map(|k| m[r][k] * v[k]).sum())
}

pub fn invert(m: &Matrix) -> Matrix {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let c =
        |r1: usize, c1: usize, r2: usize, c2: usize| m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1];
    let adj = [
        [c(1, 1, 2, 2), -c(0, 1, 2, 2), c(0, 1, 1, 2)],
        [-c(1, 0, 2, 2), c(0, 0, 2, 2), -c(0, 0, 1, 2)],
        [c(1, 0, 2, 1), -c(0, 0, 2, 1), c(0, 0, 1, 1)],
    ];
    std::array::from_fn(|r| std::array::from_fn(|k| adj[r][k] / det))
}

pub const IDENTITY: Matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Adaptation chromatique de Bradford : ce qui est blanc sous `from` devient
/// blanc sous `to` (XYZ, Y = 1).
pub fn bradford(from: [f64; 3], to: [f64; 3]) -> Matrix {
    let s = apply(&BRADFORD, from);
    let d = apply(&BRADFORD, to);
    let scale = [
        [d[0] / s[0], 0.0, 0.0],
        [0.0, d[1] / s[1], 0.0],
        [0.0, 0.0, d[2] / s[2]],
    ];
    mul(&invert(&BRADFORD), &mul(&scale, &BRADFORD))
}

pub fn xy_of(xyz: [f64; 3]) -> [f64; 2] {
    let sum = xyz[0] + xyz[1] + xyz[2];
    [xyz[0] / sum, xyz[1] / sum]
}

pub fn white_of(xy: [f64; 2]) -> [f64; 3] {
    [xy[0] / xy[1], 1.0, (1.0 - xy[0] - xy[1]) / xy[1]]
}

/// CIE 1960 (u, v), où se mesure la teinte (Duv).
fn uv_of(xy: [f64; 2]) -> [f64; 2] {
    let d = -2.0 * xy[0] + 12.0 * xy[1] + 3.0;
    [4.0 * xy[0] / d, 6.0 * xy[1] / d]
}

fn xy_of_uv(uv: [f64; 2]) -> [f64; 2] {
    let d = 2.0 * uv[0] - 8.0 * uv[1] + 4.0;
    [3.0 * uv[0] / d, 2.0 * uv[1] / d]
}

/// Lieu de Planck, approximation de Kim et al. (1667 à 25 000 K, prolongée
/// au-delà par la même formule, régulière).
fn planck_xy(t: f64) -> [f64; 2] {
    let t = t.max(1667.0);
    let (a, b, c) = (1e9 / (t * t * t), 1e6 / (t * t), 1e3 / t);
    let x = if t <= 4000.0 {
        -0.266_123_9 * a - 0.234_358_9 * b + 0.877_695_6 * c + 0.179_910
    } else {
        -3.025_846_9 * a + 2.107_037_9 * b + 0.222_634_7 * c + 0.240_390
    };
    let y = if t <= 2222.0 {
        -1.106_381_4 * x * x * x - 1.348_110_20 * x * x + 2.185_558_32 * x - 0.202_196_83
    } else if t <= 4000.0 {
        -0.954_947_6 * x * x * x - 1.374_185_93 * x * x + 2.091_370_15 * x - 0.167_488_67
    } else {
        3.081_758 * x * x * x - 5.873_386_7 * x * x + 3.751_129_9 * x - 0.370_014_83
    };
    [x, y]
}

/// Normale unitaire au lieu en (u, v), orientée vers le vert (Duv positif).
fn locus_normal(t: f64) -> [f64; 2] {
    let a = uv_of(planck_xy(t * 0.999));
    let b = uv_of(planck_xy(t * 1.001));
    let (du, dv) = (b[0] - a[0], b[1] - a[1]);
    let len = du.hypot(dv);
    // La température croît vers le bleu : la normale à gauche pointe vers le vert.
    let n = [-dv / len, du / len];
    if n[1] >= 0.0 { n } else { [-n[0], -n[1]] }
}

/// Chromaticité d'une lumière de température `t` et de teinte `tint`.
pub fn xy_from_temperature(t: f64, tint: f64) -> [f64; 2] {
    let uv = uv_of(planck_xy(t));
    let n = locus_normal(t);
    let duv = -tint / TINT_SCALE;
    xy_of_uv([uv[0] + n[0] * duv, uv[1] + n[1] * duv])
}

/// Température et teinte d'une chromaticité : pied de la perpendiculaire au
/// lieu, cherché en mireds (échelle régulière pour l'œil).
pub fn temperature_of(xy: [f64; 2]) -> (f64, f64) {
    let uv = uv_of(xy);
    let dist = |mired: f64| {
        let p = uv_of(planck_xy(1e6 / mired));
        (p[0] - uv[0]).hypot(p[1] - uv[1])
    };
    let (mut lo, mut hi) = (1e6 / TEMPERATURE_RANGE.1, 1e6 / TEMPERATURE_RANGE.0);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    for _ in 0..80 {
        let a = hi - g * (hi - lo);
        let b = lo + g * (hi - lo);
        if dist(a) < dist(b) { hi = b } else { lo = a }
    }
    let t = 1e6 / ((lo + hi) / 2.0);
    let p = uv_of(planck_xy(t));
    let n = locus_normal(t);
    let duv = (uv[0] - p[0]) * n[0] + (uv[1] - p[1]) * n[1];
    (
        t.clamp(TEMPERATURE_RANGE.0, TEMPERATURE_RANGE.1),
        (-duv * TINT_SCALE).clamp(TINT_RANGE.0, TINT_RANGE.1),
    )
}

/// Lumière visée par un réglage : `None` pour la balance du boîtier.
pub fn target_white(wb: &WhiteBalance) -> Option<[f64; 3]> {
    let (t, tint) = match wb {
        WhiteBalance::AsShot => return None,
        WhiteBalance::Preset { preset } => {
            let (_, t, tint) = PRESETS.iter().find(|(name, ..)| name == preset)?;
            (*t, *tint)
        }
        WhiteBalance::Custom { temperature, tint } => (*temperature, *tint),
    };
    Some(white_of(xy_from_temperature(t, tint)))
}

/// Correction à appliquer en XYZ à l'image développée (déjà à la balance du
/// boîtier) : on défait l'adaptation du boîtier, puis on adapte depuis la
/// lumière choisie.
pub fn correction_xyz(wb: &WhiteBalance, as_shot: [f64; 3]) -> Matrix {
    match target_white(wb) {
        None => IDENTITY,
        Some(target) => mul(&bradford(target, D65_WHITE), &bradford(D65_WHITE, as_shot)),
    }
}

/// Balance qui rend neutre une couleur de l'image développée (XYZ) : sa
/// couleur dans la scène est celle de la lumière qui l'éclaire.
pub fn neutralizing(xyz: [f64; 3], as_shot: [f64; 3]) -> WhiteBalance {
    let scene = apply(&bradford(D65_WHITE, as_shot), xyz);
    let (temperature, tint) = temperature_of(xy_of(scene));
    WhiteBalance::Custom {
        temperature: temperature.round(),
        tint: tint.round(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_et_teinte_aller_retour() {
        for t in [2500.0, 3200.0, 5000.0, 6500.0, 9000.0, 20_000.0] {
            for tint in [-40.0, 0.0, 25.0] {
                let (t2, tint2) = temperature_of(xy_from_temperature(t, tint));
                assert!((t2 - t).abs() / t < 0.002, "{t} K : {t2}");
                assert!((tint2 - tint).abs() < 0.3, "{tint} : {tint2}");
            }
        }
        // D65 : environ 6500 K et une teinte voisine de zéro (légèrement verte).
        let (t, tint) = temperature_of(xy_of(D65_WHITE));
        assert!((t - 6504.0).abs() < 30.0 && tint.abs() < 12.0, "{t} {tint}");
    }

    #[test]
    fn la_balance_du_boitier_ne_change_rien() {
        let as_shot = white_of(xy_from_temperature(4300.0, 8.0));
        let (t, tint) = temperature_of(xy_of(as_shot));
        let m = correction_xyz(
            &WhiteBalance::Custom {
                temperature: t,
                tint,
            },
            as_shot,
        );
        for (r, row) in m.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let want = if r == c { 1.0 } else { 0.0 };
                assert!((v - want).abs() < 2e-3, "{m:?}");
            }
        }
        assert_eq!(correction_xyz(&WhiteBalance::AsShot, as_shot), IDENTITY);
    }

    #[test]
    fn plus_chaud_jaunit_et_pipette_neutralise() {
        let as_shot = D65_WHITE;
        // Une température plus élevée que la lumière réelle réchauffe l'image.
        let m = correction_xyz(
            &WhiteBalance::Custom {
                temperature: 9000.0,
                tint: 0.0,
            },
            as_shot,
        );
        let white = apply(&m, D65_WHITE);
        assert!(white[2] < D65_WHITE[2] * 0.95, "{white:?}");
        // La pipette sur ce blanc réchauffé redonne la balance qui le neutralise.
        let wb = neutralizing(white, as_shot);
        let back = apply(&correction_xyz(&wb, as_shot), white);
        let xy = xy_of(back);
        let d65 = xy_of(D65_WHITE);
        assert!(
            (xy[0] - d65[0]).abs() < 2e-3 && (xy[1] - d65[1]).abs() < 2e-3,
            "{wb:?} {xy:?}"
        );
    }
}
