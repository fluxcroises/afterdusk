//! Masques des retouches locales (décision 0014) : poids de 0 à 1 de chaque
//! masque en un pixel de la sortie. Les formes sont rangées en fractions de la
//! source (avant géométrie) ; chaque pixel de la sortie est ramené au point de
//! la source qu'il montre, par le même calcul que le rééchantillonnage de
//! l'image (voir [`geometry`]). Le masque est ainsi exact à toute définition,
//! aperçu, zoom ou export, et reste aligné sur l'image recadrée ou redressée.

use super::geometry::{self, Plan};
use super::settings::{Geometry, LocalAdjust, Mask, MaskShape, Stroke};
use super::strokes::{StrokeGrid, ToOutput};

/// Montée douce de 0 à 1 sur [0, 1] (dérivée nulle aux deux bouts).
fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Forme ramenée en pixels de la source, prête à évaluer.
#[derive(Debug, Clone, PartialEq)]
enum Compiled {
    /// Origine (effet plein), direction divisée par sa longueur au carré :
    /// `t = (p - from) · dir` vaut 0 en `from` et 1 en `to`.
    Linear { from: [f64; 2], dir: [f64; 2] },
    /// Centre, cosinus et sinus de l'angle, inverses des demi-axes, et bord
    /// intérieur de l'adoucissement (distance réduite où l'effet commence à
    /// baisser).
    Radial {
        center: [f64; 2],
        cos: f64,
        sin: f64,
        inv_rx: f64,
        inv_ry: f64,
        inner: f64,
    },
    /// Pinceau : pas de forme, l'effet vient de ses tracés.
    Brush,
}

impl Compiled {
    fn new(shape: &MaskShape, sw: f64, sh: f64) -> Self {
        let px = |p: [f64; 2]| [p[0] * sw, p[1] * sh];
        match shape {
            MaskShape::Linear { from, to } => {
                let (a, b) = (px(*from), px(*to));
                let d = [b[0] - a[0], b[1] - a[1]];
                let len2 = (d[0] * d[0] + d[1] * d[1]).max(f64::EPSILON);
                Self::Linear {
                    from: a,
                    dir: [d[0] / len2, d[1] / len2],
                }
            }
            MaskShape::Radial {
                center,
                radius,
                angle,
                feather,
            } => {
                let (sin, cos) = angle.to_radians().sin_cos();
                Self::Radial {
                    center: px(*center),
                    cos,
                    sin,
                    inv_rx: 1.0 / (radius[0] * sw).max(f64::EPSILON),
                    inv_ry: 1.0 / (radius[1] * sh).max(f64::EPSILON),
                    inner: 1.0 - feather / 100.0,
                }
            }
            MaskShape::Brush => Self::Brush,
        }
    }

    /// Poids au point `p` de la source, en pixels.
    fn weight(&self, p: [f64; 2]) -> f64 {
        match *self {
            Self::Linear { from, dir } => {
                let t = (p[0] - from[0]) * dir[0] + (p[1] - from[1]) * dir[1];
                1.0 - smoothstep(t)
            }
            Self::Radial {
                center,
                cos,
                sin,
                inv_rx,
                inv_ry,
                inner,
            } => {
                // Dans le repère de l'ellipse, qui tourne de `angle` dans le
                // sens horaire (y vers le bas).
                let (dx, dy) = (p[0] - center[0], p[1] - center[1]);
                let x = (dx * cos + dy * sin) * inv_rx;
                let y = (-dx * sin + dy * cos) * inv_ry;
                let q = x.hypot(y);
                if q >= 1.0 {
                    0.0
                } else if q <= inner {
                    1.0
                } else {
                    1.0 - smoothstep((q - inner) / (1.0 - inner))
                }
            }
            Self::Brush => 0.0,
        }
    }
}

/// Forme d'un masque en pixels de la sortie entière (voir
/// [`MaskSet::planar`]) : `u` et `v` sont les coefficients de (x, y, 1), au
/// centre (x, y) d'un pixel. Linéaire : `u` vaut 0 à l'effet plein, 1 à
/// l'effet nul. Radial : (`u`, `v`) est la position dans le repère de
/// l'ellipse, de rayon 1.
#[derive(Debug, Clone, PartialEq)]
pub struct Planar {
    pub radial: bool,
    pub invert: bool,
    pub inner: f64,
    pub u: [f64; 3],
    pub v: [f64; 3],
}

impl Planar {
    /// Poids au centre (x, y) d'un pixel : le calcul du shader, en f64.
    pub fn weight(&self, x: f64, y: f64) -> f64 {
        let u = self.u[0] * x + self.u[1] * y + self.u[2];
        // Raccourcis des cas les plus fréquents (hors de l'ellipse, en deçà ou
        // au-delà du dégradé), même résultat que le calcul complet.
        let w = if self.radial {
            let v = self.v[0] * x + self.v[1] * y + self.v[2];
            let q2 = u * u + v * v;
            if q2 >= 1.0 {
                0.0
            } else if q2 <= self.inner * self.inner {
                1.0
            } else {
                1.0 - smoothstep((q2.sqrt() - self.inner) / (1.0 - self.inner))
            }
        } else if u <= 0.0 {
            1.0
        } else if u >= 1.0 {
            0.0
        } else {
            1.0 - smoothstep(u)
        };
        if self.invert { 1.0 - w } else { w }
    }
}

/// Un masque actif, prêt à évaluer, et ses réglages.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    shape: Compiled,
    invert: bool,
    pub adjust: LocalAdjust,
    /// Tracés du pinceau et de la gomme (décision 0015).
    strokes: Vec<Stroke>,
}

/// Les masques actifs d'une photo, avec le passage d'un pixel de la sortie au
/// point de la source qu'il montre.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskSet {
    pub fields: Vec<Field>,
    geometry: Geometry,
    plan: Plan,
    sw: f64,
    sh: f64,
}

impl MaskSet {
    /// Masques actifs (ni masqués, ni sans réglage) d'une source de
    /// `sw` × `sh` pixels, vue à travers `geometry`.
    pub fn new(masks: &[Mask], geometry: &Geometry, sw: u32, sh: u32) -> Self {
        Self::sized(masks, geometry, f64::from(sw), f64::from(sh))
    }

    /// Comme [`MaskSet::new`], la source mesurée en pixels de la sortie, pas
    /// forcément entiers : une sortie redimensionnée après la géométrie (export,
    /// vignettes) a une source à son échelle.
    pub fn sized(masks: &[Mask], geometry: &Geometry, sw: f64, sh: f64) -> Self {
        let (sw, sh) = (sw.max(1.0), sh.max(1.0));
        Self {
            fields: masks
                .iter()
                .filter(|m| m.is_active())
                .map(|m| Field {
                    shape: Compiled::new(&m.shape, sw, sh),
                    invert: m.invert,
                    adjust: m.adjust.clone(),
                    strokes: m.strokes.clone(),
                })
                .collect(),
            geometry: geometry.clone(),
            plan: geometry::plan(geometry, sw, sh),
            sw,
            sh,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Point de la source (en pixels) montré par le pixel (x, y) de la sortie
    /// entière, à son centre.
    pub fn source_point(&self, x: u32, y: u32) -> [f64; 2] {
        let canvas = [
            self.plan.left + f64::from(x) + 0.5,
            self.plan.top + f64::from(y) + 0.5,
        ];
        geometry::canvas_to_source(canvas, &self.geometry, self.sw, self.sh)
    }

    /// Poids du masque actif `i` au point `p` de la source (en pixels).
    pub fn weight_of(&self, i: usize, p: [f64; 2]) -> f32 {
        let f = &self.fields[i];
        let w = f.shape.weight(p);
        (if f.invert { 1.0 - w } else { w }) as f32
    }

    /// Formes ramenées en pixels de la sortie entière, pour le processeur
    /// graphique : le passage de la sortie à la source est affine (rotation,
    /// miroirs, quarts de tour, décalage du recadrage), la forme compilée
    /// aussi, leur composée se calcule une fois par masque.
    pub fn planar(&self) -> Vec<Planar> {
        let (o, ex, ey) = self.unit_points();
        // Coefficients de (x, y, 1) d'une forme affine de la source.
        let compose = |f: &dyn Fn([f64; 2]) -> f64| {
            let c = f(o);
            [f(ex) - c, f(ey) - c, c]
        };
        self.fields
            .iter()
            .map(|field| match field.shape {
                Compiled::Linear { from, dir } => Planar {
                    radial: false,
                    invert: field.invert,
                    inner: 0.0,
                    u: compose(&|p| (p[0] - from[0]) * dir[0] + (p[1] - from[1]) * dir[1]),
                    v: [0.0; 3],
                },
                Compiled::Radial {
                    center,
                    cos,
                    sin,
                    inv_rx,
                    inv_ry,
                    inner,
                } => Planar {
                    radial: true,
                    invert: field.invert,
                    inner,
                    u: compose(&|p| ((p[0] - center[0]) * cos + (p[1] - center[1]) * sin) * inv_rx),
                    v: compose(&|p| {
                        (-(p[0] - center[0]) * sin + (p[1] - center[1]) * cos) * inv_ry
                    }),
                },
                // Pinceau : toujours hors d'une ellipse, poids nul ; ses
                // tracés s'y ajoutent, l'inversion vient après eux.
                Compiled::Brush => Planar {
                    radial: true,
                    invert: false,
                    inner: 0.0,
                    u: [0.0, 0.0, 2.0],
                    v: [0.0; 3],
                },
            })
            .collect()
    }

    /// Points de la source montrés par (0, 0), (1, 0) et (0, 1) de la sortie
    /// entière : le passage de la sortie à la source, affine.
    fn unit_points(&self) -> ([f64; 2], [f64; 2], [f64; 2]) {
        let at = |x: f64, y: f64| {
            geometry::canvas_to_source(
                [self.plan.left + x, self.plan.top + y],
                &self.geometry,
                self.sw,
                self.sh,
            )
        };
        (at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0))
    }

    /// Tracés de chaque masque actif rangés en pixels de la sortie entière,
    /// ou rien pour un masque sans tracé.
    pub fn stroke_grids(&self) -> Vec<Option<StrokeGrid>> {
        let (o, ex, ey) = self.unit_points();
        let to_output = ToOutput::new(o, ex, ey);
        let (w, h) = (f64::from(self.plan.width), f64::from(self.plan.height));
        self.fields
            .iter()
            .map(|f| StrokeGrid::new(&f.strokes, &to_output, (self.sw, self.sh), w, h))
            .collect()
    }

    /// Masques dont l'inversion vient après les tracés : le pinceau, inversé,
    /// agit partout sauf là où il a peint. Un dégradé est inversé avant ses
    /// tracés, qui le gomment tel qu'il apparaît.
    pub fn invert_after_strokes(&self) -> Vec<bool> {
        self.fields
            .iter()
            .map(|f| f.invert && matches!(f.shape, Compiled::Brush))
            .collect()
    }

    /// Poids de chaque masque au pixel (x, y) de la sortie entière, dans
    /// l'ordre des masques actifs.
    pub fn weights(&self, x: u32, y: u32, out: &mut Vec<f32>) {
        out.clear();
        let p = self.source_point(x, y);
        out.extend(self.fields.iter().map(|f| {
            let w = f.shape.weight(p);
            (if f.invert { 1.0 - w } else { w }) as f32
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::settings::CropRect;

    fn mask(shape: MaskShape) -> Mask {
        Mask {
            shape,
            adjust: LocalAdjust {
                exposure: 1.0,
                ..LocalAdjust::default()
            },
            invert: false,
            hidden: false,
            strokes: Vec::new(),
        }
    }

    fn weight(set: &MaskSet, x: u32, y: u32) -> f32 {
        let mut w = Vec::new();
        set.weights(x, y, &mut w);
        w[0]
    }

    /// Ciel : plein en haut, nul à mi-hauteur, image de 400 × 200.
    fn ciel() -> Mask {
        mask(MaskShape::Linear {
            from: [0.5, 0.0],
            to: [0.5, 0.5],
        })
    }

    #[test]
    fn degrade_lineaire_plein_puis_nul() {
        let set = MaskSet::new(&[ciel()], &Geometry::default(), 400, 200);
        assert!(weight(&set, 200, 0) > 0.99);
        assert!((weight(&set, 200, 50) - 0.5).abs() < 0.02);
        assert_eq!(weight(&set, 200, 100), 0.0);
        assert_eq!(weight(&set, 200, 199), 0.0);
        // Même poids tout le long d'une ligne parallèle au bord du dégradé.
        assert_eq!(weight(&set, 0, 30), weight(&set, 399, 30));
    }

    #[test]
    fn degrade_radial_en_pixels_de_la_source() {
        // Ellipse de 100 × 100 pixels de demi-axes sur une image de 400 × 200 :
        // un cercle, malgré l'image non carrée.
        let rond = mask(MaskShape::Radial {
            center: [0.5, 0.5],
            radius: [0.25, 0.5],
            angle: 0.0,
            feather: 0.0,
        });
        let set = MaskSet::new(&[rond], &Geometry::default(), 400, 200);
        assert_eq!(weight(&set, 200, 100), 1.0);
        assert_eq!(weight(&set, 290, 100), 1.0);
        assert_eq!(weight(&set, 200, 190), 1.0);
        assert_eq!(weight(&set, 310, 100), 0.0);
        // Sans adoucissement, le bord est net.
        assert_eq!(weight(&set, 275, 175), 0.0);
    }

    #[test]
    fn adoucissement_et_inversion() {
        let doux = mask(MaskShape::Radial {
            center: [0.5, 0.5],
            radius: [0.25, 0.5],
            angle: 0.0,
            feather: 100.0,
        });
        let set = MaskSet::new(std::slice::from_ref(&doux), &Geometry::default(), 400, 200);
        let (centre, mi, bord) = (
            weight(&set, 199, 99),
            weight(&set, 250, 100),
            weight(&set, 298, 100),
        );
        assert!(
            centre > 0.99 && mi > 0.3 && mi < 0.7 && bord < 0.01,
            "{centre} {mi} {bord}"
        );
        let inverse = MaskSet::new(
            &[Mask {
                invert: true,
                ..doux
            }],
            &Geometry::default(),
            400,
            200,
        );
        assert!((weight(&inverse, 250, 100) - (1.0 - mi)).abs() < 1e-6);
        assert_eq!(weight(&inverse, 350, 100), 1.0);
    }

    #[test]
    fn degrade_radial_tourne() {
        // Ellipse allongée (80 × 10 pixels) tournée d'un quart de tour : elle
        // devient verticale.
        let fine = mask(MaskShape::Radial {
            center: [0.5, 0.5],
            radius: [0.2, 0.05],
            angle: 90.0,
            feather: 0.0,
        });
        let set = MaskSet::new(&[fine], &Geometry::default(), 400, 200);
        assert_eq!(weight(&set, 200, 160), 1.0);
        assert_eq!(weight(&set, 260, 100), 0.0);
    }

    #[test]
    fn masque_masque_ou_sans_reglage_ignore() {
        let masques = [
            Mask {
                hidden: true,
                ..ciel()
            },
            Mask {
                adjust: LocalAdjust::default(),
                ..ciel()
            },
        ];
        assert!(MaskSet::new(&masques, &Geometry::default(), 400, 200).is_empty());
    }

    #[test]
    fn le_masque_suit_le_recadrage_et_le_quart_de_tour() {
        // Recadrage sur la moitié basse : le haut de la sortie est la
        // mi-hauteur de la source, où le ciel ne porte plus.
        let bas = Geometry {
            crop: CropRect {
                left: 0.0,
                top: 0.5,
                right: 1.0,
                bottom: 1.0,
            },
            ..Geometry::default()
        };
        let set = MaskSet::new(&[ciel()], &bas, 400, 200);
        assert_eq!(weight(&set, 200, 0), 0.0);
        // Quart de tour : le haut de la source devient le bord droit de la
        // sortie (200 × 400).
        let tourne = Geometry {
            quarter: 1,
            ..Geometry::default()
        };
        let set = MaskSet::new(&[ciel()], &tourne, 400, 200);
        assert!(weight(&set, 199, 200) > 0.99, "{}", weight(&set, 199, 200));
        assert_eq!(weight(&set, 0, 200), 0.0);
        // Même point de la source que le rééchantillonnage de l'image.
        let p = set.source_point(199, 200);
        assert!(p[1] < 1.0, "{p:?}");
    }

    #[test]
    fn formes_en_pixels_de_sortie_egales_au_calcul_par_la_source() {
        let biais = Geometry {
            quarter: 3,
            angle: 7.5,
            flip_h: true,
            crop: CropRect {
                left: 0.1,
                top: 0.05,
                right: 0.8,
                bottom: 0.9,
            },
            ..Geometry::default()
        };
        let masques = [
            ciel(),
            Mask {
                invert: true,
                ..mask(MaskShape::Radial {
                    center: [0.3, 0.6],
                    radius: [0.2, 0.3],
                    angle: 25.0,
                    feather: 60.0,
                })
            },
        ];
        let set = MaskSet::new(&masques, &biais, 400, 200);
        let planar = set.planar();
        let mut w = Vec::new();
        for (x, y) in [(0, 0), (37, 91), (120, 300), (150, 12), (99, 199)] {
            set.weights(x, y, &mut w);
            for (i, p) in planar.iter().enumerate() {
                let ours = p.weight(f64::from(x) + 0.5, f64::from(y) + 0.5);
                assert!((ours - f64::from(w[i])).abs() < 1e-6, "{x},{y} masque {i}");
            }
        }
    }

    #[test]
    fn rond_de_pinceau_a_sa_place_a_travers_la_geometrie() {
        let biais = Geometry {
            quarter: 1,
            angle: -11.0,
            flip_h: true,
            crop: CropRect {
                left: 0.05,
                top: 0.1,
                right: 0.9,
                bottom: 0.95,
            },
            ..Geometry::default()
        };
        // Source de 400 × 200 : rond de rayon 20 pixels (côté long 400) en
        // (120, 70).
        let pinceau = Mask {
            shape: MaskShape::Brush,
            strokes: vec![Stroke {
                points: vec![[0.3, 0.35]],
                radius: 0.05,
                feather: 0.0,
                flow: 100.0,
                erase: false,
            }],
            ..mask(MaskShape::Brush)
        };
        let set = MaskSet::new(&[pinceau], &biais, 400, 200);
        let grids = set.stroke_grids();
        let grid = grids[0].as_ref().unwrap();
        let (w, h) = (set.plan.width, set.plan.height);
        let mut inside = 0;
        for y in 0..h {
            for x in 0..w {
                let p = set.source_point(x, y);
                let d = (p[0] - 120.0).hypot(p[1] - 70.0);
                let v = grid.apply(0.0, [f64::from(x) + 0.5, f64::from(y) + 0.5]);
                if d < 19.0 {
                    assert_eq!(v, 1.0, "({x}, {y}) à {d:.1} px du centre");
                    inside += 1;
                } else if d > 21.0 {
                    assert_eq!(v, 0.0, "({x}, {y}) à {d:.1} px du centre");
                }
            }
        }
        // Le rond est bien dans la sortie : environ π × 19² pixels.
        assert!(inside > 1000, "{inside}");
    }

    #[test]
    fn meme_point_de_la_source_que_l_image_reechantillonnee() {
        use crate::develop::pipeline::WorkingImage;
        // Chaque pixel porte sa position (centre) : l'interpolation bilinéaire
        // la rend exactement, et l'image transformée dit quel point de la
        // source chaque pixel montre.
        let (w, h) = (64u32, 40u32);
        let rgb = (0..h)
            .flat_map(|y| (0..w).flat_map(move |x| [x as f32 + 0.5, y as f32 + 0.5, 0.0]))
            .collect();
        let src = WorkingImage {
            width: w,
            height: h,
            rgb,
            as_shot_white: [0.95, 1.0, 1.09],
            scale: 1.0,
            crop: None,
            rendered: true,
        };
        let g = Geometry {
            angle: 7.0,
            crop: CropRect {
                left: 0.2,
                top: 0.15,
                right: 0.8,
                bottom: 0.85,
            },
            flip_h: true,
            ..Geometry::default()
        };
        let out = geometry::transform(&src, &g);
        let set = MaskSet::new(&[ciel()], &g, w, h);
        for (x, y) in [(3u32, 4u32), (20, 10), (30, 20)] {
            let i = ((y * out.width + x) * 3) as usize;
            let p = set.source_point(x, y);
            assert!((f64::from(out.rgb[i]) - p[0]).abs() < 1e-3, "{x},{y} {p:?}");
            assert!(
                (f64::from(out.rgb[i + 1]) - p[1]).abs() < 1e-3,
                "{x},{y} {p:?}"
            );
        }
    }
}
