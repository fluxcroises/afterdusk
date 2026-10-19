//! Courbes de tonalité par points : interpolation cubique monotone
//! (Fritsch–Carlson), qui passe par chaque point sans jamais dépasser ses
//! voisins, puis table de correspondance lue de la même façon par le
//! processeur et par le GPU.

use super::settings::{Curve, CurvePoint};

/// Entrées d'une table : 1024 pas sur [0, 1], lus par interpolation linéaire.
pub const LUT_SIZE: usize = 1024;

/// Valeurs de la courbe aux `LUT_SIZE` entrées régulières de [0, 1].
/// Moins de deux points : l'identité. Hors des points extrêmes, la courbe
/// reste à la hauteur du point le plus proche.
pub fn table(points: &[CurvePoint]) -> Vec<f32> {
    let identity = || {
        (0..LUT_SIZE)
            .map(|i| i as f32 / (LUT_SIZE - 1) as f32)
            .collect()
    };
    if points.len() < 2 {
        return identity();
    }
    let x: Vec<f64> = points.iter().map(|p| p[0] / 255.0).collect();
    let y: Vec<f64> = points.iter().map(|p| p[1] / 255.0).collect();
    let n = x.len();
    // Pentes des segments, puis tangentes aux points (Fritsch–Carlson).
    let d: Vec<f64> = (0..n - 1)
        .map(|i| (y[i + 1] - y[i]) / (x[i + 1] - x[i]).max(1e-9))
        .collect();
    let mut m = vec![0.0; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 {
            0.0
        } else {
            (d[i - 1] + d[i]) / 2.0
        };
    }
    for i in 0..n - 1 {
        if d[i] == 0.0 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
            continue;
        }
        let (a, b) = (m[i] / d[i], m[i + 1] / d[i]);
        let h = a.hypot(b);
        if h > 3.0 {
            m[i] = 3.0 * a / h * d[i];
            m[i + 1] = 3.0 * b / h * d[i];
        }
    }
    (0..LUT_SIZE)
        .map(|k| {
            let t = k as f64 / (LUT_SIZE - 1) as f64;
            let v = if t <= x[0] {
                y[0]
            } else if t >= x[n - 1] {
                y[n - 1]
            } else {
                let i = x
                    .partition_point(|&xi| xi <= t)
                    .saturating_sub(1)
                    .min(n - 2);
                let h = x[i + 1] - x[i];
                let s = (t - x[i]) / h;
                let (s2, s3) = (s * s, s * s * s);
                (2.0 * s3 - 3.0 * s2 + 1.0) * y[i]
                    + (s3 - 2.0 * s2 + s) * h * m[i]
                    + (-2.0 * s3 + 3.0 * s2) * y[i + 1]
                    + (s3 - s2) * h * m[i + 1]
            };
            v.clamp(0.0, 1.0) as f32
        })
        .collect()
}

/// Les quatre tables (RVB, rouge, vert, bout à bout), ou rien si toutes les
/// courbes sont l'identité.
pub fn tables(curve: &Curve) -> Option<Vec<f32>> {
    if curve.is_default() {
        return None;
    }
    let mut out = Vec::with_capacity(4 * LUT_SIZE);
    for points in [&curve.rgb, &curve.red, &curve.green, &curve.blue] {
        out.extend(table(points));
    }
    Some(out)
}

/// Lecture d'une table en `v` (0 à 1), par interpolation linéaire.
#[inline]
pub fn sample(lut: &[f32], v: f32) -> f32 {
    let pos = v.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32;
    let i = (pos as usize).min(LUT_SIZE - 2);
    let f = pos - i as f32;
    lut[i] * (1.0 - f) + lut[i + 1] * f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identite_et_points_respectes() {
        let id = table(&[]);
        assert!((sample(&id, 0.37) - 0.37).abs() < 1e-6);
        let s = table(&[[0.0, 0.0], [64.0, 40.0], [192.0, 215.0], [255.0, 255.0]]);
        assert!((sample(&s, 64.0 / 255.0) - 40.0 / 255.0).abs() < 2e-3);
        assert!((sample(&s, 192.0 / 255.0) - 215.0 / 255.0).abs() < 2e-3);
    }

    #[test]
    fn monotone_sans_depassement() {
        // Une marche brutale : une spline ordinaire déborderait.
        let s = table(&[[0.0, 0.0], [100.0, 10.0], [110.0, 240.0], [255.0, 255.0]]);
        assert!(s.windows(2).all(|w| w[1] >= w[0] - 1e-6));
        assert!(s.iter().all(|v| (0.0..=1.0).contains(v)));
        // Extrémités déplacées : noir relevé, blanc abaissé, à plat au-delà.
        let fade = table(&[[30.0, 20.0], [255.0, 230.0]]);
        assert!((fade[0] - 20.0 / 255.0).abs() < 1e-6);
        assert!((sample(&fade, 0.05) - 20.0 / 255.0).abs() < 1e-6);
        assert!((fade[LUT_SIZE - 1] - 230.0 / 255.0).abs() < 1e-6);
    }
}
