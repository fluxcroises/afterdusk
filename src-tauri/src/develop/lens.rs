//! Corrections d'objectifs (processus 1, décision 0006) : distorsion,
//! aberrations chromatiques latérales et vignetage, d'après la base lensfun
//! (portage Rust `lensfun`, base CC-BY-SA 3.0 incluse).
//!
//! Le vignetage est corrigé sur l'image linéaire telle que décodée ; la
//! distorsion et les aberrations en un seul rééchantillonnage, par canal. Une
//! distorsion en barillet corrigée laisserait des bords vides : l'image est
//! agrandie juste assez pour les remplir (comme « Contraindre le recadrage »).
//! Le résultat est l'image *corrigée*, repère des corrections de défauts et
//! de la géométrie, de mêmes dimensions que l'image décodée.
//!
//! Coordonnées lensfun : le centre du pixel i est en i.

use std::sync::OnceLock;
use std::thread;

use lensfun::{Database, Lens, Modifier};

use super::pipeline::{Crop, WorkingImage, workers};
use super::settings::LensSettings;
use super::spots::Fetch;
use crate::raw::RawInfo;

/// Distance de mise au point inconnue des métadonnées : l'infini, pour le
/// vignetage (comme darktable).
const FOCUS_DISTANCE: f32 = 1000.0;
/// Côté long de l'image de référence pour l'agrandissement automatique : il ne
/// dépend ainsi que des proportions, pas de la définition.
const FIT_REFERENCE: f64 = 1000.0;
/// Agrandissement au plus (barillet extrême).
const FIT_MAX: f64 = 1.6;

static DATABASE: OnceLock<Option<Database>> = OnceLock::new();

/// Base lensfun incluse, décompressée au premier usage (environ 5 Mo).
fn database() -> Option<&'static Database> {
    DATABASE
        .get_or_init(|| {
            Database::load_bundled()
                .inspect_err(|e| eprintln!("base lensfun illisible : {e}"))
                .ok()
        })
        .as_ref()
}

/// Ce qu'il faut savoir de la prise de vue pour trouver le profil.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shot {
    pub make: String,
    pub model: String,
    pub lens: Option<String>,
    pub focal: Option<f32>,
    pub aperture: Option<f32>,
}

impl From<&RawInfo> for Shot {
    fn from(i: &RawInfo) -> Self {
        Self {
            make: i.make.clone(),
            model: i.model.clone(),
            lens: i.lens.clone(),
            focal: i.focal_length,
            aperture: i.aperture,
        }
    }
}

/// D'après l'EXIF d'une photo déjà développée (JPEG, TIFF, PNG, HEIF).
impl From<&crate::metadata::media::MediaMetadata> for Shot {
    fn from(m: &crate::metadata::media::MediaMetadata) -> Self {
        Self {
            make: m.make.clone().unwrap_or_default(),
            model: m.model.clone().unwrap_or_default(),
            lens: m.lens.clone(),
            focal: m.focal_length,
            aperture: m.aperture,
        }
    }
}

/// Objectif reconnu et paramètres de la prise de vue.
#[derive(Debug, Clone)]
pub struct LensProfile {
    lens: Lens,
    crop: f32,
    focal: f32,
    aperture: f32,
    /// Nom affiché : marque et modèle de l'objectif dans la base.
    pub name: String,
}

/// Profil de l'objectif d'une prise de vue, ou rien s'il n'est pas reconnu.
pub fn find_profile(shot: &Shot) -> Option<LensProfile> {
    let db = database()?;
    let focal = shot.focal.filter(|f| *f > 0.0)?;
    let camera = *db.find_cameras(Some(&shot.make), &shot.model).first()?;
    let name = shot.lens.as_deref().map_or("", str::trim);
    let lens = if name.is_empty() {
        // Compact à objectif fixe : l'objectif propre à sa monture, sans
        // complément optique (« + GW-4 »). Une monture interchangeable sans nom
        // d'objectif ne dit rien de l'objectif monté : pas de correction.
        let own: Vec<&Lens> = db
            .find_lenses(Some(camera), "")
            .into_iter()
            .filter(|l| l.mounts.contains(&camera.mount) && !l.model.contains('+'))
            .collect();
        match own.as_slice() {
            [only] => *only,
            _ => return None,
        }
    } else {
        *db.find_lenses(Some(camera), name).first()?
    };
    // Focale hors de celles de l'objectif : ce n'est pas lui (un complément
    // optique, une erreur de reconnaissance).
    if lens.focal_min > 0.0
        && (focal < lens.focal_min * 0.97 || focal > lens.focal_max.max(lens.focal_min) * 1.03)
    {
        return None;
    }
    if lens.calib_distortion.is_empty()
        && lens.calib_tca.is_empty()
        && lens.calib_vignetting.is_empty()
    {
        return None;
    }
    let display = if lens.maker.is_empty() || lens.model.starts_with(&lens.maker) {
        lens.model.clone()
    } else {
        format!("{} {}", lens.maker, lens.model)
    };
    Some(LensProfile {
        lens: lens.clone(),
        crop: camera.crop_factor,
        focal,
        aperture: shot
            .aperture
            .filter(|a| *a > 0.0)
            .unwrap_or(lens.aperture_min),
        name: display,
    })
}

/// Centre optique (pixels) d'une image w × h, comme `Modifier::new`.
fn center(lens: &Lens, w: u32, h: u32) -> (f64, f64) {
    let width = f64::from(w.max(2) - 1);
    let height = f64::from(h.max(2) - 1);
    let size = width.min(height);
    (
        width / 2.0 + size / 2.0 * f64::from(lens.center_x),
        height / 2.0 + size / 2.0 * f64::from(lens.center_y),
    )
}

/// Corrections à appliquer, d'après le profil et les réglages.
#[derive(Debug, Clone)]
pub struct LensCorrection {
    profile: LensProfile,
    /// Part de la distorsion corrigée (1 : le profil).
    distortion: f64,
    /// Part du vignetage corrigé (1 : le profil).
    vignetting: f32,
    /// Agrandissement qui remplit les bords, en fonction des proportions.
    fit: f64,
}

impl LensCorrection {
    /// `None` : corrections désactivées, ou rien à corriger. `rendered` :
    /// photo déjà développée (corrections désactivées par défaut).
    pub fn new(profile: &LensProfile, settings: &LensSettings, rendered: bool) -> Option<Self> {
        if !settings.active(rendered) {
            return None;
        }
        let mut c = Self {
            profile: profile.clone(),
            distortion: settings.distortion / 100.0,
            vignetting: (settings.vignetting / 100.0) as f32,
            fit: 1.0,
        };
        if !c.has_geometry() && !c.has_vignetting() {
            return None;
        }
        c.fit = c.fit_for(1.5);
        Some(c)
    }

    fn has_distortion(&self) -> bool {
        self.distortion > 0.0 && !self.profile.lens.calib_distortion.is_empty()
    }

    fn has_tca(&self) -> bool {
        !self.profile.lens.calib_tca.is_empty()
    }

    fn has_geometry(&self) -> bool {
        self.has_distortion() || self.has_tca()
    }

    fn has_vignetting(&self) -> bool {
        self.vignetting > 0.0 && !self.profile.lens.calib_vignetting.is_empty()
    }

    /// Modificateur de distorsion et d'aberrations pour une image w × h.
    fn geometry_modifier(&self, w: u32, h: u32) -> Modifier {
        let p = &self.profile;
        let mut m = Modifier::new(&p.lens, p.focal, p.crop, w, h, true);
        if self.has_distortion() {
            m.enable_distortion_correction(&p.lens);
        }
        if self.has_tca() {
            m.enable_tca_correction(&p.lens);
        }
        m
    }

    /// Image virtuelle agrandie de `k` : la même optique vue sur un capteur
    /// plus grand, ce qui revient à agrandir l'image corrigée.
    fn virtual_size(w: u32, h: u32, k: f64) -> (u32, u32) {
        (
            ((f64::from(w) * k).round() as u32).max(2),
            ((f64::from(h) * k).round() as u32).max(2),
        )
    }

    /// Positions dans l'image décodée (w × h), par canal [xR, yR, xG, yG, xB,
    /// yB], des pixels (x0..x0+cols, y) de l'image corrigée agrandie de `k`.
    #[allow(clippy::too_many_arguments)]
    fn row_positions(
        &self,
        m: &Modifier,
        (w, h): (u32, u32),
        (vw, vh): (u32, u32),
        x0: u32,
        y: u32,
        cols: usize,
        out: &mut Vec<[f32; 6]>,
    ) {
        let lens = &self.profile.lens;
        let (cx, cy) = center(lens, w, h);
        let (vcx, vcy) = center(lens, vw, vh);
        let k = f64::from(vw).hypot(f64::from(vh)) / f64::from(w).hypot(f64::from(h));
        let (sx, sy) = (f64::from(x0) + vcx - cx, f64::from(y) + vcy - cy);
        let back = |vx: f32, vy: f32| {
            [
                cx + (f64::from(vx) - vcx) / k,
                cy + (f64::from(vy) - vcy) / k,
            ]
        };
        let mut geo = vec![0.0f32; cols * 2];
        let mut tca = vec![0.0f32; cols * 6];
        let has_geo = m.apply_geometry_distortion(sx as f32, sy as f32, cols, 1, &mut geo);
        let has_tca = m.apply_subpixel_distortion(sx as f32, sy as f32, cols, 1, &mut tca);
        out.clear();
        for i in 0..cols {
            // Point sans correction : le pixel ramené par l'agrandissement.
            let q = back((sx + i as f64) as f32, sy as f32);
            let g = if has_geo {
                let r = back(geo[2 * i], geo[2 * i + 1]);
                [
                    q[0] + self.distortion * (r[0] - q[0]),
                    q[1] + self.distortion * (r[1] - q[1]),
                ]
            } else {
                q
            };
            let mut px = [0.0f32; 6];
            for c in 0..3 {
                let shift = if has_tca {
                    let r = back(tca[6 * i + 2 * c], tca[6 * i + 2 * c + 1]);
                    [r[0] - q[0], r[1] - q[1]]
                } else {
                    [0.0, 0.0]
                };
                px[2 * c] = (g[0] + shift[0]) as f32;
                px[2 * c + 1] = (g[1] + shift[1]) as f32;
            }
            out.push(px);
        }
    }

    /// Les bords de l'image corrigée, agrandie de `k`, tombent-ils tous dans
    /// l'image décodée ?
    fn edges_inside(&self, w: u32, h: u32, k: f64) -> bool {
        let v = Self::virtual_size(w, h, k);
        let m = self.geometry_modifier(v.0, v.1);
        let (xmax, ymax) = (f64::from(w) - 0.5, f64::from(h) - 0.5);
        let mut row = Vec::new();
        let inside = |row: &[[f32; 6]]| {
            row.iter().all(|p| {
                (0..3).all(|c| {
                    let (x, y) = (f64::from(p[2 * c]), f64::from(p[2 * c + 1]));
                    x >= -0.5 && y >= -0.5 && x <= xmax && y <= ymax
                })
            })
        };
        for y in [0, h - 1] {
            self.row_positions(&m, (w, h), v, 0, y, w as usize, &mut row);
            if !inside(&row) {
                return false;
            }
        }
        for x in [0, w - 1] {
            for y in (0..h).step_by(4) {
                self.row_positions(&m, (w, h), v, x, y, 1, &mut row);
                if !inside(&row) {
                    return false;
                }
            }
        }
        true
    }

    /// Plus petit agrandissement sans bord vide, pour des proportions
    /// largeur / hauteur `aspect`.
    fn fit_for(&self, aspect: f64) -> f64 {
        if !self.has_distortion() {
            return 1.0;
        }
        let (w, h) = if aspect >= 1.0 {
            (FIT_REFERENCE, FIT_REFERENCE / aspect)
        } else {
            (FIT_REFERENCE * aspect, FIT_REFERENCE)
        };
        let (w, h) = (w.round() as u32, h.round() as u32);
        if self.edges_inside(w, h, 1.0) {
            return 1.0;
        }
        let (mut lo, mut hi) = (1.0, FIT_MAX);
        if !self.edges_inside(w, h, hi) {
            return hi;
        }
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if self.edges_inside(w, h, mid) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }

    /// Prépare l'agrandissement pour les proportions de l'image (le profil
    /// est calculé pour 3:2 à la création).
    pub fn for_image(mut self, w: u32, h: u32) -> Self {
        self.fit = self.fit_for(f64::from(w) / f64::from(h.max(1)));
        self
    }

    /// Corrige le vignetage de `img` (l'image décodée entière, ou une région,
    /// voir [`WorkingImage::frame`]), sur place.
    pub fn vignette(&self, img: &mut WorkingImage) {
        if !self.has_vignetting() {
            return;
        }
        let p = &self.profile;
        let (ox, oy, fw, fh) = img.frame();
        // Pour le vignetage, `reverse = false` corrige (convention de
        // lensfun, inverse de celle de la distorsion).
        let mut m = Modifier::new(&p.lens, p.focal, p.crop, fw, fh, false);
        if !m.enable_vignetting_correction(&p.lens, p.aperture, FOCUS_DISTANCE) {
            return;
        }
        let width = img.width as usize;
        let rows_per = (img.height as usize).div_ceil(workers()).max(1);
        let amount = self.vignetting;
        thread::scope(|s| {
            for (k, chunk) in img.rgb.chunks_mut(rows_per * width * 3).enumerate() {
                let m = &m;
                s.spawn(move || {
                    let mut gain = vec![1.0f32; width];
                    for (r, line) in chunk.chunks_mut(width * 3).enumerate() {
                        let y = oy as usize + k * rows_per + r;
                        gain.fill(1.0);
                        m.apply_color_modification_f32(&mut gain, ox as f32, y as f32, width, 1, 1);
                        for (px, g) in line.chunks_mut(3).zip(&gain) {
                            let g = if amount == 1.0 { *g } else { g.powf(amount) };
                            for v in px {
                                *v *= g;
                            }
                        }
                    }
                });
            }
        });
    }

    /// La région (left, top, width, height) de l'image corrigée, de taille
    /// fw × fh. `read` lit l'image décodée (sans correction) ; le vignetage
    /// est corrigé sur ce qu'elle renvoie.
    pub fn region(
        &self,
        fw: u32,
        fh: u32,
        rect: (u32, u32, u32, u32),
        read: &Fetch,
    ) -> WorkingImage {
        let (l, t, w, h) = rect;
        let whole = l == 0 && t == 0 && w == fw && h == fh;
        if !self.has_geometry() {
            let mut img = read(l, t, w, h);
            self.vignette(&mut img);
            return img;
        }
        let v = Self::virtual_size(fw, fh, self.fit);
        let m = self.geometry_modifier(v.0, v.1);
        // Rectangle de l'image décodée à lire : celui des bords de la région
        // (la correction est continue et sans repli).
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut row = Vec::new();
        let mut grow = |row: &[[f32; 6]]| {
            for p in row {
                for c in 0..3 {
                    x0 = x0.min(f64::from(p[2 * c]));
                    x1 = x1.max(f64::from(p[2 * c]));
                    y0 = y0.min(f64::from(p[2 * c + 1]));
                    y1 = y1.max(f64::from(p[2 * c + 1]));
                }
            }
        };
        for y in [t, t + h - 1] {
            self.row_positions(&m, (fw, fh), v, l, y, w as usize, &mut row);
            grow(&row);
        }
        for x in [l, l + w - 1] {
            for y in t..t + h {
                self.row_positions(&m, (fw, fh), v, x, y, 1, &mut row);
                grow(&row);
            }
        }
        let clamp_x = |v: f64| v.clamp(0.0, f64::from(fw)) as u32;
        let clamp_y = |v: f64| v.clamp(0.0, f64::from(fh)) as u32;
        let (rx0, ry0) = (clamp_x(x0.floor() - 2.0), clamp_y(y0.floor() - 2.0));
        let (rx1, ry1) = (clamp_x(x1.ceil() + 3.0), clamp_y(y1.ceil() + 3.0));
        let mut src = read(rx0, ry0, (rx1 - rx0).max(1), (ry1 - ry0).max(1));
        self.vignette(&mut src);

        let mut rgb = vec![0.0f32; (w * h * 3) as usize];
        let rows_per = (h as usize).div_ceil(workers()).max(1);
        thread::scope(|s| {
            for (k, chunk) in rgb.chunks_mut(rows_per * w as usize * 3).enumerate() {
                let (m, src) = (&m, &src);
                s.spawn(move || {
                    let mut pos = Vec::with_capacity(w as usize);
                    for (r, line) in chunk.chunks_mut(w as usize * 3).enumerate() {
                        let y = t + (k * rows_per + r) as u32;
                        self.row_positions(m, (fw, fh), v, l, y, w as usize, &mut pos);
                        for (px, p) in line.chunks_mut(3).zip(&pos) {
                            for (c, out) in px.iter_mut().enumerate() {
                                *out = sample(src, p[2 * c], p[2 * c + 1], c);
                            }
                        }
                    }
                });
            }
        });
        WorkingImage {
            width: w,
            height: h,
            rgb,
            as_shot_white: src.as_shot_white,
            rendered: src.rendered,
            scale: src.scale,
            crop: (!whole).then_some(Crop {
                left: l,
                top: t,
                full_width: fw,
                full_height: fh,
            }),
        }
    }

    /// L'image décodée entière corrigée (aperçu).
    pub fn whole(&self, src: &WorkingImage) -> WorkingImage {
        let (w, h) = (src.width, src.height);
        self.region(w, h, (0, 0, w, h), &|l, t, rw, rh| {
            if l == 0 && t == 0 && rw == w && rh == h {
                src.clone()
            } else {
                src.sub(l, t, rw, rh)
            }
        })
    }
}

/// Canal `c` de `src` (région d'une image plus grande) au point (x, y) de
/// l'image entière, centres de pixel aux entiers ; bords répétés.
fn sample(src: &WorkingImage, x: f32, y: f32, c: usize) -> f32 {
    let (ox, oy, _, _) = src.frame();
    let lx = (x - ox as f32).clamp(0.0, (src.width - 1) as f32);
    let ly = (y - oy as f32).clamp(0.0, (src.height - 1) as f32);
    let (x0, y0) = (lx.floor() as u32, ly.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(src.width - 1), (y0 + 1).min(src.height - 1));
    let (fx, fy) = (lx - x0 as f32, ly - y0 as f32);
    let at = |xx: u32, yy: u32| src.rgb[((yy * src.width + xx) * 3) as usize + c];
    let top = at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx;
    let bottom = at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx;
    top * (1.0 - fy) + bottom * fy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(make: &str, model: &str, lens: Option<&str>, focal: f32, aperture: f32) -> Shot {
        Shot {
            make: make.into(),
            model: model.into(),
            lens: lens.map(Into::into),
            focal: Some(focal),
            aperture: Some(aperture),
        }
    }

    fn sony() -> LensProfile {
        find_profile(&shot(
            "Sony",
            "ILCE-7M3",
            Some("FE 16-35mm F2.8 GM"),
            16.0,
            2.8,
        ))
        .unwrap()
    }

    #[test]
    fn objectifs_des_echantillons_reconnus() {
        let name = |s: Shot| find_profile(&s).map(|p| p.name);
        assert_eq!(
            name(shot(
                "Sony",
                "ILCE-7M3",
                Some("FE 16-35mm F2.8 GM"),
                32.0,
                4.0
            ))
            .as_deref(),
            Some("Sony FE 16-35mm f/2.8 GM")
        );
        assert!(name(shot("Fujifilm", "X-T3", Some("XF35mmF2 R WR"), 35.0, 3.6)).is_some());
        assert!(
            name(shot(
                "Nikon",
                "Z 6",
                Some("NIKKOR Z 24-70mm f/4 S"),
                52.0,
                6.7
            ))
            .is_some()
        );
        assert!(
            name(shot(
                "Canon",
                "EOS R6",
                Some("RF70-200mm F2.8 L IS USM"),
                200.0,
                25.0
            ))
            .is_some()
        );
        // Compact : la base n'a pour le GR III que l'objectif avec complément
        // grand angle GW-4 (13,8 mm) ; à 18,3 mm, ce n'est pas lui.
        assert_eq!(name(shot("Ricoh", "GR III", None, 18.3, 5.0)), None);
        assert!(name(shot("Ricoh", "GR III", None, 13.8, 5.0)).is_none());
        // L'objectif fixe d'un compact reconnu sans nom d'objectif.
        let gr = name(shot("Ricoh", "GR", None, 18.3, 5.0)).unwrap();
        assert!(gr.contains("GR"), "{gr}");
        // Monture interchangeable sans nom d'objectif : rien.
        assert_eq!(name(shot("Panasonic", "DC-G9", None, 40.0, 2.8)), None);
        // Sans focale, rien non plus.
        let mut no_focal = shot("Sony", "ILCE-7M3", Some("FE 16-35mm F2.8 GM"), 16.0, 2.8);
        no_focal.focal = None;
        assert!(find_profile(&no_focal).is_none());
    }

    /// Image de 300 × 200 : dégradés et quadrillage.
    fn pattern(w: u32, h: u32) -> WorkingImage {
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let grid = if x % 20 == 0 || y % 20 == 0 { 0.2 } else { 0.0 };
                let v = 0.3 + 0.3 * x as f32 / w as f32 + grid;
                rgb.extend_from_slice(&[v, 0.5 - 0.2 * y as f32 / h as f32, v * 0.5]);
            }
        }
        WorkingImage {
            width: w,
            height: h,
            rgb,
            as_shot_white: [0.95, 1.0, 1.09],
            scale: 1.0,
            crop: None,
            rendered: false,
        }
    }

    fn correction(distortion: f64, vignetting: f64) -> LensCorrection {
        let s = LensSettings {
            enabled: Some(true),
            distortion,
            vignetting,
        };
        LensCorrection::new(&sony(), &s, false)
            .unwrap()
            .for_image(300, 200)
    }

    #[test]
    fn region_identique_a_l_image_entiere() {
        let img = pattern(300, 200);
        let c = correction(100.0, 100.0);
        let whole = c.whole(&img);
        assert_eq!((whole.width, whole.height), (300, 200));
        let read = |l, t, w, h| img.sub(l, t, w, h);
        let part = c.region(300, 200, (40, 150, 70, 50), &read);
        for y in 0..50 {
            for x in 0..70 {
                for ch in 0..3 {
                    let a = part.rgb[((y * 70 + x) * 3 + ch) as usize];
                    let b = whole.rgb[(((y + 150) * 300 + x + 40) * 3 + ch) as usize];
                    assert!((a - b).abs() < 1e-5, "({x}, {y}) : {a} contre {b}");
                }
            }
        }
    }

    #[test]
    fn barillet_corrige_sans_bord_vide() {
        let c = correction(100.0, 0.0);
        // 16 mm : barillet marqué, l'image est agrandie.
        assert!(c.fit > 1.0 && c.fit < 1.2, "{}", c.fit);
        assert!(c.edges_inside(300, 200, c.fit));
        assert!(!c.edges_inside(300, 200, 1.0));
        // Même agrandissement à toute définition (proportions égales).
        assert!((c.clone().for_image(6000, 4000).fit - c.fit).abs() < 1e-9);
        // Sans correction de distorsion, pas d'agrandissement.
        assert_eq!(correction(0.0, 0.0).fit, 1.0);
    }

    #[test]
    fn vignetage_eclaircit_les_coins() {
        let mut img = pattern(300, 200);
        img.rgb.fill(0.4);
        correction(0.0, 100.0).vignette(&mut img);
        let at = |x: u32, y: u32| img.rgb[((y * 300 + x) * 3) as usize];
        assert!((at(150, 100) - 0.4).abs() < 0.01);
        assert!(at(0, 0) > 0.45, "{}", at(0, 0));
        // À moitié : entre les deux.
        let mut half = pattern(300, 200);
        half.rgb.fill(0.4);
        correction(0.0, 50.0).vignette(&mut half);
        let h = half.rgb[0];
        assert!(h > 0.4 && h < at(0, 0));
    }

    #[test]
    fn desactivee_ou_sans_effet() {
        let off = LensSettings {
            enabled: Some(false),
            ..LensSettings::default()
        };
        assert!(LensCorrection::new(&sony(), &off, false).is_none());
    }
}
