//! Tracés du pinceau et de la gomme (décision 0015) : poids de 0 à 1 d'un
//! masque en un pixel de la sortie, sans image intermédiaire. Les tracés sont
//! ramenés une fois en pixels de la sortie (le passage de la source à la
//! sortie ne fait que tourner, retourner, déplacer et agrandir : un rond de
//! pinceau reste un rond) ; en un pixel, un tracé vaut sa distance à la ligne
//! brisée de ses points, adoucie et multipliée par sa densité. Une grille
//! range les segments par case : chaque pixel ne regarde que ceux de la
//! sienne.

use super::settings::Stroke;

/// Montée douce de 0 à 1 sur [0, 1].
fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Passage affine d'un point de la source (pixels) à la sortie entière
/// (pixels) : `out = m · (p - o)`, et rapport des longueurs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToOutput {
    /// Point de la source montré par le coin (0, 0) de la sortie.
    pub origin: [f64; 2],
    /// Inverse de la matrice sortie → source.
    pub m: [[f64; 2]; 2],
    /// Pixels de la sortie par pixel de la source.
    pub scale: f64,
}

impl ToOutput {
    /// D'après les points de la source montrés par (0, 0), (1, 0) et (0, 1)
    /// de la sortie.
    pub fn new(o: [f64; 2], ex: [f64; 2], ey: [f64; 2]) -> Self {
        let (a, c) = (ex[0] - o[0], ex[1] - o[1]);
        let (b, d) = (ey[0] - o[0], ey[1] - o[1]);
        let det = a * d - b * c;
        let det = if det.abs() < f64::EPSILON { 1.0 } else { det };
        Self {
            origin: o,
            m: [[d / det, -b / det], [-c / det, a / det]],
            scale: 1.0 / det.abs().sqrt(),
        }
    }

    fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        let (dx, dy) = (p[0] - self.origin[0], p[1] - self.origin[1]);
        [
            self.m[0][0] * dx + self.m[0][1] * dy,
            self.m[1][0] * dx + self.m[1][1] * dy,
        ]
    }
}

/// Segment d'un tracé, en pixels de la sortie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub radius: f64,
    /// Distance réduite où l'effet commence à baisser.
    pub inner: f64,
    /// Densité, de 0,01 à 1.
    pub flow: f64,
    pub erase: bool,
    /// Indice du tracé dans le masque : les segments d'un même tracé se
    /// suivent dans chaque case.
    pub stroke: u32,
}

impl Segment {
    /// Effet du tracé au point `p` (avant densité), de 0 à 1.
    fn profile(&self, p: [f64; 2]) -> f64 {
        let (dx, dy) = (self.b[0] - self.a[0], self.b[1] - self.a[1]);
        let (px, py) = (p[0] - self.a[0], p[1] - self.a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            ((px * dx + py * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (ex, ey) = (px - t * dx, py - t * dy);
        let d2 = (ex * ex + ey * ey) / (self.radius * self.radius);
        if d2 >= 1.0 {
            0.0
        } else if d2 <= self.inner * self.inner {
            1.0
        } else {
            1.0 - smoothstep((d2.sqrt() - self.inner) / (1.0 - self.inner))
        }
    }
}

/// Plus petit et plus grand côté d'une case, en pixels de la sortie.
const CELL_MIN: f64 = 8.0;
const CELL_MAX: f64 = 512.0;
/// Cases au plus dans la grille.
const CELLS_MAX: f64 = (1 << 20) as f64;

/// Tracés d'un masque, rangés par case de la sortie.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeGrid {
    pub segments: Vec<Segment>,
    /// Coin haut gauche de la grille et côté d'une case, en pixels de la sortie.
    pub origin: [f64; 2],
    pub cell: f64,
    pub cols: u32,
    pub rows: u32,
    /// Segments de la case `i` : `entries[starts[i]..starts[i + 1]]`, dans
    /// l'ordre des tracés.
    pub starts: Vec<u32>,
    pub entries: Vec<u32>,
}

impl StrokeGrid {
    /// Tracés ramenés en pixels de la sortie (`width` × `height`), ou rien
    /// s'il n'y en a pas, pour une source de `sw` × `sh` pixels.
    pub fn new(
        strokes: &[Stroke],
        to_output: &ToOutput,
        (sw, sh): (f64, f64),
        width: f64,
        height: f64,
    ) -> Option<Self> {
        let long_side = sw.max(sh);
        let mut segments = Vec::new();
        for (k, s) in strokes.iter().enumerate() {
            let radius = (s.radius * long_side * to_output.scale).max(0.5);
            let inner = 1.0 - s.feather / 100.0;
            let mut push = |a: [f64; 2], b: [f64; 2]| {
                segments.push(Segment {
                    a,
                    b,
                    radius,
                    inner,
                    flow: s.flow / 100.0,
                    erase: s.erase,
                    stroke: k as u32,
                });
            };
            // Points trop proches du précédent (moins d'un quart de rayon)
            // sautés : la ligne brisée garde sa forme.
            let mut points = s
                .points
                .iter()
                .map(|p| to_output.apply([p[0] * sw, p[1] * sh]));
            let Some(first) = points.next() else { continue };
            let mut last = first;
            let mut drawn = false;
            for p in points {
                if (p[0] - last[0]).hypot(p[1] - last[1]) >= radius * 0.25 {
                    push(last, p);
                    last = p;
                    drawn = true;
                }
            }
            if !drawn {
                push(first, first);
            }
        }
        if segments.is_empty() {
            return None;
        }
        // Grille sur les segments visibles ; une case de la taille du rayon
        // le plus courant.
        let mut radii: Vec<f64> = segments.iter().map(|s| s.radius).collect();
        radii.sort_by(f64::total_cmp);
        let mut cell = radii[radii.len() / 2].clamp(CELL_MIN, CELL_MAX);
        let (x0, y0, x1, y1) = segments.iter().fold(
            (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
            |(x0, y0, x1, y1), s| {
                (
                    x0.min(s.a[0].min(s.b[0]) - s.radius),
                    y0.min(s.a[1].min(s.b[1]) - s.radius),
                    x1.max(s.a[0].max(s.b[0]) + s.radius),
                    y1.max(s.a[1].max(s.b[1]) + s.radius),
                )
            },
        );
        let (x0, y0) = (x0.max(0.0), y0.max(0.0));
        let (x1, y1) = (x1.min(width), y1.min(height));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        while ((x1 - x0) / cell).ceil() * ((y1 - y0) / cell).ceil() > CELLS_MAX {
            cell *= 2.0;
        }
        let cols = ((x1 - x0) / cell).ceil().max(1.0) as u32;
        let rows = ((y1 - y0) / cell).ceil().max(1.0) as u32;
        let range = |lo: f64, hi: f64, n: u32| {
            let a = (lo / cell).floor().max(0.0) as u32;
            let b = ((hi / cell).floor().max(0.0) as u32).min(n - 1);
            a..=b
        };
        let mut lists = vec![Vec::new(); (cols * rows) as usize];
        for (i, s) in segments.iter().enumerate() {
            let xs = range(
                s.a[0].min(s.b[0]) - s.radius - x0,
                s.a[0].max(s.b[0]) + s.radius - x0,
                cols,
            );
            let ys = range(
                s.a[1].min(s.b[1]) - s.radius - y0,
                s.a[1].max(s.b[1]) + s.radius - y0,
                rows,
            );
            for y in ys {
                for x in xs.clone() {
                    lists[(y * cols + x) as usize].push(i as u32);
                }
            }
        }
        let mut starts = Vec::with_capacity(lists.len() + 1);
        let mut entries = Vec::new();
        for l in &lists {
            starts.push(entries.len() as u32);
            entries.extend_from_slice(l);
        }
        starts.push(entries.len() as u32);
        Some(Self {
            segments,
            origin: [x0, y0],
            cell,
            cols,
            rows,
            starts,
            entries,
        })
    }

    /// Poids du masque au point `p` de la sortie (pixels), en partant de
    /// `base` : chaque tracé, dans l'ordre, ajoute son effet sans dépasser le
    /// plein effet, ou le retire (gomme).
    pub fn apply(&self, base: f64, p: [f64; 2]) -> f64 {
        let (cx, cy) = (
            (p[0] - self.origin[0]) / self.cell,
            (p[1] - self.origin[1]) / self.cell,
        );
        if cx < 0.0 || cy < 0.0 || cx >= f64::from(self.cols) || cy >= f64::from(self.rows) {
            return base;
        }
        let c = (cy as u32 * self.cols + cx as u32) as usize;
        let list = &self.entries[self.starts[c] as usize..self.starts[c + 1] as usize];
        let mut m = base;
        let mut current: Option<(u32, f64, bool)> = None;
        let mut s = 0.0f64;
        let combine = |m: f64, s: f64, flow: f64, erase: bool| {
            let s = s * flow;
            if erase {
                m * (1.0 - s)
            } else {
                m + s * (1.0 - m)
            }
        };
        for &i in list {
            let seg = &self.segments[i as usize];
            match current {
                Some((k, _, _)) if k == seg.stroke => {}
                Some((_, flow, erase)) => {
                    m = combine(m, s, flow, erase);
                    s = 0.0;
                    current = Some((seg.stroke, seg.flow, seg.erase));
                }
                None => current = Some((seg.stroke, seg.flow, seg.erase)),
            }
            if s < 1.0 {
                s = s.max(seg.profile(p));
            }
        }
        if let Some((_, flow, erase)) = current {
            m = combine(m, s, flow, erase);
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: ToOutput = ToOutput {
        origin: [0.0, 0.0],
        m: [[1.0, 0.0], [0.0, 1.0]],
        scale: 1.0,
    };

    /// Tracé en pixels d'une source de 400 × 400 (côté long 400).
    fn trait_(points: &[[f64; 2]], radius_px: f64, flow: f64, erase: bool) -> Stroke {
        Stroke {
            points: points
                .iter()
                .map(|p| [p[0] / 400.0, p[1] / 400.0])
                .collect(),
            radius: radius_px / 400.0,
            feather: 0.0,
            flow,
            erase,
        }
    }

    fn grille(strokes: &[Stroke]) -> StrokeGrid {
        StrokeGrid::new(strokes, &IDENTITY, (400.0, 400.0), 400.0, 400.0).unwrap()
    }

    #[test]
    fn trait_plein_dans_son_rayon_nul_au_dela() {
        let g = grille(&[trait_(
            &[[100.0, 100.0], [300.0, 100.0]],
            20.0,
            100.0,
            false,
        )]);
        assert_eq!(g.apply(0.0, [200.0, 110.0]), 1.0);
        assert_eq!(g.apply(0.0, [100.0, 100.0]), 1.0);
        assert_eq!(g.apply(0.0, [200.0, 125.0]), 0.0);
        // Bouts arrondis : un rond au premier et au dernier point.
        assert_eq!(g.apply(0.0, [85.0, 100.0]), 1.0);
        assert_eq!(g.apply(0.0, [75.0, 100.0]), 0.0);
        // Loin de tout tracé : hors de la grille, la base passe telle quelle.
        assert_eq!(g.apply(0.3, [390.0, 390.0]), 0.3);
    }

    #[test]
    fn densite_et_passages_successifs() {
        let un = [trait_(&[[200.0, 200.0]], 30.0, 50.0, false)];
        assert!((grille(&un).apply(0.0, [200.0, 200.0]) - 0.5).abs() < 1e-12);
        // Deux passages à 50 % : 75 %, jamais plus que le plein effet.
        let deux = [un[0].clone(), un[0].clone()];
        assert!((grille(&deux).apply(0.0, [200.0, 200.0]) - 0.75).abs() < 1e-12);
        // Un même tracé qui repasse sur lui-même ne s'accumule pas.
        let boucle = [trait_(
            &[[150.0, 200.0], [250.0, 200.0], [150.0, 200.0]],
            30.0,
            50.0,
            false,
        )];
        assert!((grille(&boucle).apply(0.0, [200.0, 200.0]) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn gomme_retire_dans_l_ordre_des_traces() {
        let ajout = trait_(&[[200.0, 200.0]], 30.0, 100.0, false);
        let gomme = trait_(&[[200.0, 200.0]], 30.0, 100.0, true);
        // Sur un dégradé (base 0,8) : la gomme ramène à zéro.
        assert_eq!(
            grille(std::slice::from_ref(&gomme)).apply(0.8, [200.0, 200.0]),
            0.0
        );
        // Gomme à 50 % : moitié de l'effet.
        let demi = Stroke {
            flow: 50.0,
            ..gomme.clone()
        };
        assert!((grille(&[demi]).apply(0.8, [200.0, 200.0]) - 0.4).abs() < 1e-12);
        // Ajouter puis gommer : rien ; gommer puis ajouter : plein effet.
        assert_eq!(
            grille(&[ajout.clone(), gomme.clone()]).apply(0.0, [200.0, 200.0]),
            0.0
        );
        assert_eq!(grille(&[gomme, ajout]).apply(0.0, [200.0, 200.0]), 1.0);
    }

    #[test]
    fn adoucissement_du_bord() {
        let doux = Stroke {
            feather: 100.0,
            ..trait_(&[[200.0, 200.0]], 40.0, 100.0, false)
        };
        let g = grille(&[doux]);
        let (centre, mi, bord) = (
            g.apply(0.0, [200.0, 200.0]),
            g.apply(0.0, [220.0, 200.0]),
            g.apply(0.0, [239.0, 200.0]),
        );
        assert!(
            centre > 0.99 && mi > 0.3 && mi < 0.7 && bord < 0.01,
            "{centre} {mi} {bord}"
        );
    }

    #[test]
    fn long_trace_et_petites_cases() {
        // 4 000 points d'une spirale : la grille garde peu de segments par case.
        let points: Vec<[f64; 2]> = (0..4000)
            .map(|i| {
                let t = f64::from(i) * 0.01;
                [200.0 + t * 4.0 * t.cos(), 200.0 + t * 4.0 * t.sin()]
            })
            .collect();
        let g = grille(&[trait_(&points, 3.0, 100.0, false)]);
        let longest = g.starts.windows(2).map(|w| w[1] - w[0]).max().unwrap();
        assert!(longest < 200, "{longest} segments dans une case");
        assert_eq!(g.apply(0.0, [200.0, 200.0]), 1.0);
    }
}
