//! Masques des retouches locales (décision 0014) : poids de 0 à 1 de chaque
//! masque en un pixel de la sortie. Les formes sont rangées en fractions de la
//! source (avant géométrie) ; chaque pixel de la sortie est ramené au point de
//! la source qu'il montre, par le même calcul que le rééchantillonnage de
//! l'image (voir [`geometry`]). Le masque est ainsi exact à toute définition,
//! aperçu, zoom ou export, et reste aligné sur l'image recadrée ou redressée.

use super::geometry::{self, Plan};
use super::settings::{Geometry, LocalAdjust, Mask, MaskShape};

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
        }
    }
}

/// Un masque actif, prêt à évaluer, et ses réglages.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    shape: Compiled,
    invert: bool,
    pub adjust: LocalAdjust,
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
        let (sw, sh) = (f64::from(sw.max(1)), f64::from(sh.max(1)));
        Self {
            fields: masks
                .iter()
                .filter(|m| m.is_active())
                .map(|m| Field {
                    shape: Compiled::new(&m.shape, sw, sh),
                    invert: m.invert,
                    adjust: m.adjust.clone(),
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
