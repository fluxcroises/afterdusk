//! Réglages de retouche (décision 0005) : un objet JSON creux, par sections,
//! avec `version` (forme du JSON, migrée à la lecture) et `process` (calcul du
//! rendu, figé pour une photo déjà retouchée).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Forme actuelle du JSON. Toute évolution ajoute une étape à [`migrate`].
/// Version 2 : masques des retouches locales (décision 0014). Un document
/// sans masque est encore écrit en version 1, que les versions antérieures
/// d'Afterdusk savent lire (voir [`DevelopSettings::to_json`]).
pub const FORMAT_VERSION: u32 = 2;

/// Processus de rendu le plus récent, pris par les photos retouchées pour la
/// première fois. Le moteur garde le calcul de chaque processus.
pub const LATEST_PROCESS: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("réglages illisibles : {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "réglages en version {found}, plus récente que cette version d'Afterdusk ({supported})"
    )]
    TooNewFormat { found: u32, supported: u32 },
    #[error(
        "processus de rendu {found} inconnu de cette version d'Afterdusk (au plus {supported})"
    )]
    TooNewProcess { found: u32, supported: u32 },
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// Lumière : en IL pour l'exposition, de -100 à +100 pour le reste. La
/// luminosité éclaircit ou assombrit les tons moyens sans déplacer le blanc
/// ni le noir (ajoutée en V0.2, absente = 0 : pas de nouvelle version).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Light {
    #[serde(skip_serializing_if = "is_zero")]
    pub exposure: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub brightness: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub contrast: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub highlights: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub shadows: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub whites: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub blacks: f64,
}

impl Light {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Balance des blancs : celle du boîtier, un préréglage, ou kelvins et teinte.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum WhiteBalance {
    #[default]
    AsShot,
    Preset {
        preset: String,
    },
    Custom {
        temperature: f64,
        tint: f64,
    },
}

impl WhiteBalance {
    pub fn is_as_shot(&self) -> bool {
        *self == Self::AsShot
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Color {
    #[serde(skip_serializing_if = "WhiteBalance::is_as_shot")]
    pub white_balance: WhiteBalance,
    #[serde(skip_serializing_if = "is_zero")]
    pub vibrance: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub saturation: f64,
}

impl Color {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Point d'une courbe de tonalité : entrée et sortie de 0 à 255, comme
/// dans les logiciels de développement.
pub type CurvePoint = [f64; 2];

/// Nombre de points d'une courbe, extrémités comprises.
pub const CURVE_POINTS_MAX: usize = 16;

// `skip_serializing_if` reçoit une référence au type du champ, ici un `Vec`.
#[allow(clippy::ptr_arg)]
fn is_identity_curve(points: &Vec<CurvePoint>) -> bool {
    points.is_empty()
}

/// Courbes de tonalité par points : l'ensemble RVB puis chaque canal. Une
/// courbe vide est l'identité.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Curve {
    #[serde(skip_serializing_if = "is_identity_curve")]
    pub rgb: Vec<CurvePoint>,
    #[serde(skip_serializing_if = "is_identity_curve")]
    pub red: Vec<CurvePoint>,
    #[serde(skip_serializing_if = "is_identity_curve")]
    pub green: Vec<CurvePoint>,
    #[serde(skip_serializing_if = "is_identity_curve")]
    pub blue: Vec<CurvePoint>,
}

impl Curve {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Une valeur de -100 à +100 pour chacune des huit teintes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HueSet {
    #[serde(skip_serializing_if = "is_zero")]
    pub red: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub orange: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub yellow: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub green: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub aqua: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub blue: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub purple: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub magenta: f64,
}

impl HueSet {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Les huit valeurs, dans l'ordre du cercle des teintes.
    pub fn values(&self) -> [f64; 8] {
        [
            self.red,
            self.orange,
            self.yellow,
            self.green,
            self.aqua,
            self.blue,
            self.purple,
            self.magenta,
        ]
    }

    pub fn values_mut(&mut self) -> [&mut f64; 8] {
        [
            &mut self.red,
            &mut self.orange,
            &mut self.yellow,
            &mut self.green,
            &mut self.aqua,
            &mut self.blue,
            &mut self.purple,
            &mut self.magenta,
        ]
    }
}

/// Teinte, saturation et luminance par couleur (TSL).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hsl {
    #[serde(skip_serializing_if = "HueSet::is_default")]
    pub hue: HueSet,
    #[serde(skip_serializing_if = "HueSet::is_default")]
    pub saturation: HueSet,
    #[serde(skip_serializing_if = "HueSet::is_default")]
    pub luminance: HueSet,
}

impl Hsl {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

fn fifty() -> f64 {
    50.0
}

fn twenty_five() -> f64 {
    25.0
}

fn is_fifty(v: &f64) -> bool {
    *v == 50.0
}

fn is_twenty_five(v: &f64) -> bool {
    *v == 25.0
}

/// Vignetage : intensité de -100 (assombrit les coins) à +100 (les
/// éclaircit) ; point milieu et progressivité de 0 à 100.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Vignette {
    #[serde(skip_serializing_if = "is_zero")]
    pub amount: f64,
    #[serde(skip_serializing_if = "is_fifty")]
    pub midpoint: f64,
    #[serde(skip_serializing_if = "is_fifty")]
    pub feather: f64,
}

impl Default for Vignette {
    fn default() -> Self {
        Self {
            amount: 0.0,
            midpoint: fifty(),
            feather: fifty(),
        }
    }
}

/// Grain : quantité, taille et irrégularité de 0 à 100.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Grain {
    #[serde(skip_serializing_if = "is_zero")]
    pub amount: f64,
    #[serde(skip_serializing_if = "is_twenty_five")]
    pub size: f64,
    #[serde(skip_serializing_if = "is_fifty")]
    pub roughness: f64,
}

impl Default for Grain {
    fn default() -> Self {
        Self {
            amount: 0.0,
            size: twenty_five(),
            roughness: fifty(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Effects {
    #[serde(skip_serializing_if = "Vignette::is_default")]
    pub vignette: Vignette,
    #[serde(skip_serializing_if = "Grain::is_default")]
    pub grain: Grain,
}

impl Vignette {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl Grain {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl Effects {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

fn one() -> f64 {
    1.0
}

fn is_one(v: &f64) -> bool {
    *v == 1.0
}

/// Netteté : quantité de 0 à 150, rayon de 0,5 à 3 pixels de l'original,
/// détail (retenue des halos) et masquage (zones unies épargnées) de 0 à 100.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Sharpening {
    #[serde(skip_serializing_if = "is_zero")]
    pub amount: f64,
    #[serde(skip_serializing_if = "is_one")]
    pub radius: f64,
    #[serde(skip_serializing_if = "is_twenty_five")]
    pub detail: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub masking: f64,
}

impl Default for Sharpening {
    fn default() -> Self {
        Self {
            amount: 0.0,
            radius: one(),
            detail: twenty_five(),
            masking: 0.0,
        }
    }
}

/// Réduction du bruit, de 0 à 100 : luminance (et part du détail gardée),
/// couleur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NoiseReduction {
    #[serde(skip_serializing_if = "is_zero")]
    pub luminance: f64,
    #[serde(skip_serializing_if = "is_fifty")]
    pub luminance_detail: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub color: f64,
}

impl Default for NoiseReduction {
    fn default() -> Self {
        Self {
            luminance: 0.0,
            luminance_detail: fifty(),
            color: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Detail {
    #[serde(skip_serializing_if = "Sharpening::is_default")]
    pub sharpening: Sharpening,
    #[serde(skip_serializing_if = "NoiseReduction::is_default")]
    pub noise: NoiseReduction,
}

impl Sharpening {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl NoiseReduction {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl Detail {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Rectangle de recadrage en fractions de l'image orientée et redressée
/// (0 à 1), indépendant de la définition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropRect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl CropRect {
    pub const FULL: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    };

    pub fn is_full(&self) -> bool {
        *self == Self::FULL
    }
}

impl Default for CropRect {
    fn default() -> Self {
        Self::FULL
    }
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

/// Géométrie : quarts de tour (sens horaire) et miroirs, puis redressement
/// (degrés, positif dans le sens horaire), puis recadrage. `aspect` retient le
/// ratio choisi dans l'outil (« 3:2 », « original »…), pour le rouvrir.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Geometry {
    #[serde(skip_serializing_if = "CropRect::is_full")]
    pub crop: CropRect,
    #[serde(skip_serializing_if = "is_zero")]
    pub angle: f64,
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub quarter: u8,
    #[serde(skip_serializing_if = "is_false")]
    pub flip_h: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub flip_v: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect: Option<String>,
}

impl Geometry {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Même image rendue (le ratio retenu n'en change rien).
    pub fn same_image(&self, other: &Self) -> bool {
        self.crop == other.crop
            && self.angle == other.angle
            && self.quarter == other.quarter
            && self.flip_h == other.flip_h
            && self.flip_v == other.flip_v
    }

    /// Ni rotation, ni miroir, ni recadrage.
    pub fn is_identity(&self) -> bool {
        self.same_image(&Self::default())
    }
}

fn hundred() -> f64 {
    100.0
}

fn is_hundred(v: &f64) -> bool {
    *v == 100.0
}

/// Corrections d'objectifs (décision 0006) : distorsion et vignetage dosés
/// de 0 à 200 % du profil ; les aberrations chromatiques suivent `enabled`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LensSettings {
    /// Absent : le choix par défaut de la source (voir [`Self::active`]). Un
    /// document antérieur sans ce champ voulait dire « activé » : c'est le
    /// défaut des RAW, seuls retouchables alors, et il se relit à l'identique.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default = "hundred", skip_serializing_if = "is_hundred")]
    pub distortion: f64,
    #[serde(default = "hundred", skip_serializing_if = "is_hundred")]
    pub vignetting: f64,
}

impl Default for LensSettings {
    fn default() -> Self {
        Self {
            enabled: None,
            distortion: 100.0,
            vignetting: 100.0,
        }
    }
}

impl LensSettings {
    /// Corrections appliquées ? Par défaut oui pour un RAW, non pour une
    /// photo déjà développée (`rendered`), souvent déjà corrigée par le
    /// boîtier ou le logiciel qui l'a produite.
    pub fn active(&self, rendered: bool) -> bool {
        self.enabled.unwrap_or(!rendered)
    }

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Preset appliqué en dernier (galerie de filtres) et son intensité, de 0 à
/// 100 % : il se règle de nouveau sans perdre le reste de la photo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    pub id: String,
    pub amount: f64,
}

/// Mode d'une correction de défaut.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpotMode {
    /// La source, raccordée aux bords de la destination.
    #[default]
    Heal,
    /// La source copiée telle quelle.
    Clone,
}

impl SpotMode {
    fn is_heal(&self) -> bool {
        *self == Self::Heal
    }
}

/// Correction de défaut (poussière, petite imperfection) : un disque remplacé
/// par un disque voisin. Positions en fractions de la largeur et de la hauteur
/// de l'image avant géométrie (orientation du boîtier appliquée), rayon en
/// fraction de son côté long.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spot {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub source_x: f64,
    pub source_y: f64,
    /// Adoucissement du bord, de 0 à 100.
    #[serde(default = "fifty", skip_serializing_if = "is_fifty")]
    pub feather: f64,
    #[serde(default, skip_serializing_if = "SpotMode::is_heal")]
    pub mode: SpotMode,
}

/// Forme d'un masque de retouche locale. Positions en fractions de la
/// largeur et de la hauteur de l'image avant géométrie, comme les corrections
/// de défauts : le masque suit le recadrage et le redressement. Une forme peut
/// déborder de l'image (poignée posée hors cadre).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MaskShape {
    /// Dégradé linéaire : effet plein du côté de `from`, nul au-delà de `to`.
    Linear { from: [f64; 2], to: [f64; 2] },
    /// Dégradé radial : ellipse de centre `center`, de demi-axes `radius`
    /// (fractions de la largeur et de la hauteur), tournée de `angle` degrés.
    /// Effet plein à l'intérieur, adouci vers le bord selon `feather` (0 à 100).
    #[serde(rename_all = "camelCase")]
    Radial {
        center: [f64; 2],
        radius: [f64; 2],
        #[serde(default, skip_serializing_if = "is_zero")]
        angle: f64,
        #[serde(default = "fifty", skip_serializing_if = "is_fifty")]
        feather: f64,
    },
}

/// Réglages d'un masque, ajoutés à ceux de la photo là où il agit :
/// exposition en IL, le reste de -100 à +100 (température et teinte en
/// décalage relatif, comme pour une photo déjà développée).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LocalAdjust {
    #[serde(skip_serializing_if = "is_zero")]
    pub exposure: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub contrast: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub highlights: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub shadows: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub temperature: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub tint: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub saturation: f64,
}

impl LocalAdjust {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Masque de retouche locale : une forme, ses réglages, inversé ou non,
/// masqué (sans effet) ou non.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mask {
    pub shape: MaskShape,
    #[serde(default, skip_serializing_if = "LocalAdjust::is_default")]
    pub adjust: LocalAdjust,
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
}

impl Mask {
    /// Le masque change-t-il le rendu ?
    pub fn is_active(&self) -> bool {
        !self.hidden && !self.adjust.is_default()
    }
}

/// Nombre de masques au plus par photo.
pub const MASKS_MAX: usize = 32;
/// Plus petit demi-axe d'un dégradé radial, plus petite longueur d'un
/// dégradé linéaire, en fraction de l'image.
pub const MASK_SIZE_MIN: f64 = 0.001;

/// Rayon d'une correction, en fraction du côté long.
pub const SPOT_RADIUS_MIN: f64 = 0.001;
pub const SPOT_RADIUS_MAX: f64 = 0.1;
/// Nombre de corrections au plus par photo.
pub const SPOTS_MAX: usize = 1000;

/// Plus petit côté d'un recadrage, en fraction de l'image.
pub const CROP_MIN: f64 = 0.01;

/// Points d'une courbe ramenés dans [0, 255], triés, une seule sortie par
/// entrée ; moins de deux points : l'identité.
fn sanitize_curve(points: &mut Vec<CurvePoint>) {
    points.retain(|p| p[0].is_finite() && p[1].is_finite());
    for p in points.iter_mut() {
        p[0] = p[0].clamp(0.0, 255.0);
        p[1] = p[1].clamp(0.0, 255.0);
    }
    points.sort_by(|a, b| a[0].total_cmp(&b[0]));
    points.dedup_by(|a, b| a[0] == b[0]);
    points.truncate(CURVE_POINTS_MAX);
    let identity = points.len() == 2 && points[0] == [0.0, 0.0] && points[1] == [255.0, 255.0];
    if points.len() < 2 || identity {
        points.clear();
    }
}

/// Réglages d'une version de photo (l'originale ou une copie virtuelle).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopSettings {
    pub version: u32,
    pub process: u32,
    #[serde(default, skip_serializing_if = "Light::is_default")]
    pub light: Light,
    #[serde(default, skip_serializing_if = "Color::is_default")]
    pub color: Color,
    #[serde(default, skip_serializing_if = "Curve::is_default")]
    pub curve: Curve,
    #[serde(default, skip_serializing_if = "Hsl::is_default")]
    pub hsl: Hsl,
    #[serde(default, skip_serializing_if = "Detail::is_default")]
    pub detail: Detail,
    #[serde(default, skip_serializing_if = "Effects::is_default")]
    pub effects: Effects,
    #[serde(default, skip_serializing_if = "Geometry::is_default")]
    pub geometry: Geometry,
    #[serde(default, skip_serializing_if = "LensSettings::is_default")]
    pub lens: LensSettings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<Look>,
    /// Corrections de défauts, dans l'ordre où elles sont appliquées.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spots: Vec<Spot>,
    /// Masques des retouches locales, dans l'ordre où ils s'appliquent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masks: Vec<Mask>,
}

impl Default for DevelopSettings {
    /// Rendu neutre, dernier processus : une photo jamais retouchée.
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            process: LATEST_PROCESS,
            light: Light::default(),
            color: Color::default(),
            curve: Curve::default(),
            hsl: Hsl::default(),
            detail: Detail::default(),
            geometry: Geometry::default(),
            effects: Effects::default(),
            lens: LensSettings::default(),
            look: None,
            spots: Vec::new(),
            masks: Vec::new(),
        }
    }
}

fn clamp(v: f64, min: f64, max: f64) -> f64 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        0.0
    }
}

/// Fait passer un JSON d'une version ancienne à la forme actuelle, étape par étape.
fn migrate(value: Value, from: u32) -> Value {
    // Version 1 → 2 : les masques s'ajoutent ; un document v1 n'en a pas,
    // rien à transformer. Une version 3 ajoutera ici `if from < 3 { … }`,
    // testée sur un JSON v2.
    let _ = from;
    value
}

impl DevelopSettings {
    /// Lit un JSON de réglages, quelle que soit sa version (migrée), et ramène
    /// chaque valeur dans sa plage.
    pub fn from_json(text: &str) -> Result<Self, SettingsError> {
        let value: Value = serde_json::from_str(text)?;
        let version = value
            .get("version")
            .and_then(Value::as_u64)
            .map_or(1, |v| u32::try_from(v).unwrap_or(u32::MAX));
        if version > FORMAT_VERSION {
            return Err(SettingsError::TooNewFormat {
                found: version,
                supported: FORMAT_VERSION,
            });
        }
        let mut value = migrate(value, version);
        if let Some(obj) = value.as_object_mut() {
            obj.insert("version".into(), FORMAT_VERSION.into());
            obj.entry("process").or_insert(LATEST_PROCESS.into());
        }
        let settings: Self = serde_json::from_value(value)?;
        if settings.process > LATEST_PROCESS {
            return Err(SettingsError::TooNewProcess {
                found: settings.process,
                supported: LATEST_PROCESS,
            });
        }
        Ok(settings.sanitized())
    }

    /// JSON compact, sans les valeurs par défaut, dans la plus ancienne
    /// version qui suffit : sans masque, la version 1, que les versions
    /// antérieures d'Afterdusk savent lire ; avec, la version 2, qu'elles
    /// refusent au lieu d'effacer les masques en enregistrant.
    pub fn to_json(&self) -> String {
        let version = if self.masks.is_empty() { 1 } else { 2 };
        let written = Self {
            version,
            ..self.clone()
        };
        serde_json::to_string(&written).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Valeurs ramenées dans leurs plages (décision 0005).
    pub fn sanitized(mut self) -> Self {
        let l = &mut self.light;
        l.exposure = clamp(l.exposure, -5.0, 5.0);
        for v in [
            &mut l.brightness,
            &mut l.contrast,
            &mut l.highlights,
            &mut l.shadows,
            &mut l.whites,
            &mut l.blacks,
        ] {
            *v = clamp(*v, -100.0, 100.0);
        }
        let c = &mut self.color;
        c.vibrance = clamp(c.vibrance, -100.0, 100.0);
        c.saturation = clamp(c.saturation, -100.0, 100.0);
        if let WhiteBalance::Custom { temperature, tint } = &mut c.white_balance {
            *temperature = clamp(*temperature, 2000.0, 50_000.0);
            *tint = clamp(*tint, -150.0, 150.0);
        }
        let k = &mut self.curve;
        for points in [&mut k.rgb, &mut k.red, &mut k.green, &mut k.blue] {
            sanitize_curve(points);
        }
        let h = &mut self.hsl;
        for set in [&mut h.hue, &mut h.saturation, &mut h.luminance] {
            for v in set.values_mut() {
                *v = clamp(*v, -100.0, 100.0);
            }
        }
        let sh = &mut self.detail.sharpening;
        sh.amount = clamp(sh.amount, 0.0, 150.0);
        sh.radius = if sh.radius.is_finite() {
            sh.radius.clamp(0.5, 3.0)
        } else {
            1.0
        };
        sh.detail = clamp(sh.detail, 0.0, 100.0);
        sh.masking = clamp(sh.masking, 0.0, 100.0);
        let nr = &mut self.detail.noise;
        nr.luminance = clamp(nr.luminance, 0.0, 100.0);
        nr.luminance_detail = clamp(nr.luminance_detail, 0.0, 100.0);
        nr.color = clamp(nr.color, 0.0, 100.0);
        let g = &mut self.geometry;
        g.angle = if g.angle.is_finite() {
            g.angle.clamp(-45.0, 45.0)
        } else {
            0.0
        };
        g.quarter %= 4;
        let c = &mut g.crop;
        let fix = |v: f64, fallback: f64| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                fallback
            }
        };
        c.left = fix(c.left, 0.0);
        c.top = fix(c.top, 0.0);
        c.right = fix(c.right, 1.0);
        c.bottom = fix(c.bottom, 1.0);
        if c.right - c.left < CROP_MIN || c.bottom - c.top < CROP_MIN {
            *c = CropRect::FULL;
        }
        if g.aspect
            .as_ref()
            .is_some_and(|a| a.is_empty() || a.len() > 16)
        {
            g.aspect = None;
        }
        let v = &mut self.effects.vignette;
        v.amount = clamp(v.amount, -100.0, 100.0);
        v.midpoint = clamp(v.midpoint, 0.0, 100.0);
        v.feather = clamp(v.feather, 0.0, 100.0);
        let g = &mut self.effects.grain;
        g.amount = clamp(g.amount, 0.0, 100.0);
        g.size = clamp(g.size, 0.0, 100.0);
        g.roughness = clamp(g.roughness, 0.0, 100.0);
        let l = &mut self.lens;
        l.distortion = if l.distortion.is_finite() {
            l.distortion.clamp(0.0, 200.0)
        } else {
            100.0
        };
        l.vignetting = if l.vignetting.is_finite() {
            l.vignetting.clamp(0.0, 200.0)
        } else {
            100.0
        };
        if let Some(look) = &mut self.look {
            look.amount = clamp(look.amount, 0.0, 100.0);
            if look.id.is_empty() || look.id.len() > 64 {
                self.look = None;
            }
        }
        self.spots.retain(|s| {
            [s.x, s.y, s.radius, s.source_x, s.source_y]
                .iter()
                .all(|v| v.is_finite())
        });
        self.spots.truncate(SPOTS_MAX);
        for s in &mut self.spots {
            for v in [&mut s.x, &mut s.y, &mut s.source_x, &mut s.source_y] {
                *v = v.clamp(0.0, 1.0);
            }
            s.radius = s.radius.clamp(SPOT_RADIUS_MIN, SPOT_RADIUS_MAX);
            s.feather = clamp(s.feather, 0.0, 100.0);
        }
        self.masks.retain(Mask::is_valid);
        self.masks.truncate(MASKS_MAX);
        for m in &mut self.masks {
            m.sanitize();
        }
        self
    }

    pub fn is_neutral(&self) -> bool {
        self.light.is_default()
            && self.color.is_default()
            && self.curve.is_default()
            && self.hsl.is_default()
            && self.detail.is_default()
            && self.geometry.is_identity()
            && self.effects.is_default()
            && self.lens.is_default()
            && self.spots.is_empty()
            && !self.masks.iter().any(Mask::is_active)
    }
}

impl Mask {
    /// Toutes les positions finies, et une forme qui ne soit pas réduite à
    /// un point.
    fn is_valid(&self) -> bool {
        match &self.shape {
            MaskShape::Linear { from, to } => {
                from.iter().chain(to).all(|v| v.is_finite())
                    && (to[0] - from[0]).hypot(to[1] - from[1]) >= MASK_SIZE_MIN
            }
            MaskShape::Radial {
                center,
                radius,
                angle,
                feather,
            } => center
                .iter()
                .chain(radius)
                .chain([angle, feather])
                .all(|v| v.is_finite()),
        }
    }

    /// Positions ramenées autour de l'image (une image de part et d'autre),
    /// tailles et réglages dans leurs plages.
    fn sanitize(&mut self) {
        let around = |v: &mut f64| *v = v.clamp(-1.0, 2.0);
        match &mut self.shape {
            MaskShape::Linear { from, to } => {
                from.iter_mut().chain(to.iter_mut()).for_each(around);
            }
            MaskShape::Radial {
                center,
                radius,
                angle,
                feather,
            } => {
                center.iter_mut().for_each(around);
                for r in radius.iter_mut() {
                    *r = r.clamp(MASK_SIZE_MIN, 2.0);
                }
                // Angle ramené dans [-180, 180).
                *angle = (*angle + 180.0).rem_euclid(360.0) - 180.0;
                *feather = clamp(*feather, 0.0, 100.0);
            }
        }
        let a = &mut self.adjust;
        a.exposure = clamp(a.exposure, -4.0, 4.0);
        for v in [
            &mut a.contrast,
            &mut a.highlights,
            &mut a.shadows,
            &mut a.temperature,
            &mut a.tint,
            &mut a.saturation,
        ] {
            *v = clamp(*v, -100.0, 100.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_d_objectif_par_defaut_selon_la_source() {
        let d = LensSettings::default();
        assert!(d.active(false), "RAW : activée par défaut");
        assert!(
            !d.active(true),
            "photo déjà développée : désactivée par défaut"
        );
        let on = LensSettings {
            enabled: Some(true),
            ..LensSettings::default()
        };
        assert!(on.active(true));
        // Documents antérieurs : champ absent (activé) ou désactivé, relus tels quels.
        let old: LensSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.enabled, None);
        let off: LensSettings = serde_json::from_str(r#"{"enabled":false}"#).unwrap();
        assert_eq!(off.enabled, Some(false));
        assert_eq!(
            serde_json::to_string(&LensSettings::default()).unwrap(),
            "{}"
        );
    }

    #[test]
    fn json_creux_et_aller_retour() {
        let neutral = DevelopSettings::default();
        assert_eq!(neutral.to_json(), r#"{"version":1,"process":1}"#);

        let mut s = DevelopSettings::default();
        s.light.exposure = 0.35;
        s.color.white_balance = WhiteBalance::Custom {
            temperature: 5600.0,
            tint: 4.0,
        };
        let json = s.to_json();
        assert_eq!(
            json,
            r#"{"version":1,"process":1,"light":{"exposure":0.35},"color":{"whiteBalance":{"mode":"custom","temperature":5600.0,"tint":4.0}}}"#
        );
        assert_eq!(DevelopSettings::from_json(&json).unwrap(), s);
    }

    #[test]
    fn valeurs_ramenees_dans_leur_plage() {
        let s = DevelopSettings::from_json(
            r#"{"version":1,"process":1,"light":{"exposure":12,"contrast":-300},
                "color":{"whiteBalance":{"mode":"custom","temperature":100,"tint":900}}}"#,
        )
        .unwrap();
        assert_eq!(s.light.exposure, 5.0);
        assert_eq!(s.light.contrast, -100.0);
        assert_eq!(
            s.color.white_balance,
            WhiteBalance::Custom {
                temperature: 2000.0,
                tint: 150.0
            }
        );
    }

    #[test]
    fn sections_intermediaires_creuses_et_bornees() {
        let s = DevelopSettings::from_json(
            r#"{"version":1,"curve":{"rgb":[[255,240],[0,12],[128,140],[128,150],[300,-4]]},
                "hsl":{"saturation":{"blue":-40,"red":250}},
                "effects":{"vignette":{"amount":-30},"grain":{"amount":20,"size":25}}}"#,
        )
        .unwrap();
        // Triés, bornés, une sortie par entrée (le premier point donné gagne).
        assert_eq!(s.curve.rgb, [[0.0, 12.0], [128.0, 140.0], [255.0, 240.0]]);
        assert_eq!(
            (s.hsl.saturation.blue, s.hsl.saturation.red),
            (-40.0, 100.0)
        );
        assert_eq!(s.effects.vignette.midpoint, 50.0);
        // Valeurs par défaut absentes du JSON.
        let json = s.to_json();
        assert!(!json.contains("midpoint") && !json.contains("size") && !json.contains("green"));
        assert_eq!(DevelopSettings::from_json(&json).unwrap(), s);
        let d = DevelopSettings::from_json(
            r#"{"version":1,"detail":{"sharpening":{"amount":400,"radius":9},"noise":{"color":25}}}"#,
        )
        .unwrap();
        assert_eq!(
            (d.detail.sharpening.amount, d.detail.sharpening.radius),
            (150.0, 3.0)
        );
        assert_eq!(d.detail.sharpening.detail, 25.0);
        assert_eq!(
            d.to_json(),
            r#"{"version":1,"process":1,"detail":{"sharpening":{"amount":150.0,"radius":3.0},"noise":{"color":25.0}}}"#
        );
        let g = DevelopSettings::from_json(
            r#"{"version":1,"geometry":{"crop":{"left":0.1,"top":0.2,"right":0.9,"bottom":0.8},"angle":-70,"quarter":5,"flipH":true,"aspect":"3:2"}}"#,
        )
        .unwrap();
        assert_eq!((g.geometry.angle, g.geometry.quarter), (-45.0, 1));
        assert_eq!(g.geometry.aspect.as_deref(), Some("3:2"));
        assert!(!g.is_neutral());
        // Recadrage dégénéré : l'image entière.
        let d = DevelopSettings::from_json(
            r#"{"version":1,"geometry":{"crop":{"left":0.5,"top":0.2,"right":0.5,"bottom":0.8}}}"#,
        )
        .unwrap();
        assert!(d.geometry.crop.is_full());
        // Une courbe identité ou d'un seul point n'est pas enregistrée.
        let id = DevelopSettings::from_json(
            r#"{"version":1,"curve":{"rgb":[[0,0],[255,255]],"red":[[10,20]]}}"#,
        )
        .unwrap();
        assert!(id.is_neutral());
        assert_eq!(id.to_json(), r#"{"version":1,"process":1}"#);
    }

    #[test]
    fn version_ou_processus_trop_recents_refuses() {
        assert!(matches!(
            DevelopSettings::from_json(r#"{"version":3,"process":1}"#),
            Err(SettingsError::TooNewFormat { found: 3, .. })
        ));
        assert!(matches!(
            DevelopSettings::from_json(r#"{"version":1,"process":9}"#),
            Err(SettingsError::TooNewProcess { found: 9, .. })
        ));
        // Sans processus noté : le plus récent ; sections inconnues ignorées.
        let s = DevelopSettings::from_json(r#"{"version":1,"light":{"shadows":20}}"#).unwrap();
        assert_eq!((s.process, s.light.shadows), (LATEST_PROCESS, 20.0));
    }

    #[test]
    fn corrections_de_defauts_creuses_et_bornees() {
        let s = DevelopSettings::from_json(
            r#"{"version":1,"spots":[{"x":0.5,"y":0.25,"radius":0.01,"sourceX":0.6,"sourceY":0.25},{"x":1.5,"y":-1,"radius":2,"sourceX":0.1,"sourceY":0.2,"feather":20,"mode":"clone"}]}"#,
        )
        .unwrap();
        assert_eq!(s.spots.len(), 2);
        assert_eq!(
            (s.spots[0].feather, s.spots[0].mode),
            (50.0, SpotMode::Heal)
        );
        let b = &s.spots[1];
        assert_eq!((b.x, b.y, b.radius), (1.0, 0.0, SPOT_RADIUS_MAX));
        assert_eq!(b.mode, SpotMode::Clone);
        assert!(!s.is_neutral());
        // Valeurs par défaut absentes du JSON.
        assert_eq!(
            s.to_json(),
            r#"{"version":1,"process":1,"spots":[{"x":0.5,"y":0.25,"radius":0.01,"sourceX":0.6,"sourceY":0.25},{"x":1.0,"y":0.0,"radius":0.1,"sourceX":0.1,"sourceY":0.2,"feather":20.0,"mode":"clone"}]}"#
        );
    }

    fn degrade_radial() -> Mask {
        Mask {
            shape: MaskShape::Radial {
                center: [0.5, 0.4],
                radius: [0.2, 0.3],
                angle: 15.0,
                feather: 50.0,
            },
            adjust: LocalAdjust {
                exposure: 0.7,
                shadows: 20.0,
                ..LocalAdjust::default()
            },
            invert: false,
            hidden: false,
        }
    }

    #[test]
    fn document_v1_relu_sans_masque() {
        let s = DevelopSettings::from_json(
            r#"{"version":1,"process":1,"light":{"exposure":0.5},"spots":[]}"#,
        )
        .unwrap();
        assert!(s.masks.is_empty());
        assert_eq!(s.light.exposure, 0.5);
        // Réécrit sans masque : toujours lisible par les versions antérieures.
        assert!(
            s.to_json().starts_with(r#"{"version":1,"#),
            "{}",
            s.to_json()
        );
    }

    #[test]
    fn masques_en_version_2_aller_retour() {
        let s = DevelopSettings {
            masks: vec![
                Mask {
                    shape: MaskShape::Linear {
                        from: [0.5, 0.0],
                        to: [0.5, 0.45],
                    },
                    adjust: LocalAdjust {
                        exposure: -0.8,
                        temperature: -10.0,
                        ..LocalAdjust::default()
                    },
                    invert: false,
                    hidden: false,
                },
                degrade_radial(),
            ],
            ..DevelopSettings::default()
        };
        let json = s.to_json();
        assert!(json.starts_with(r#"{"version":2,"#), "{json}");
        assert!(json.contains(r#""kind":"linear""#), "{json}");
        // Valeurs par défaut absentes : adoucissement de 50, ni inversé ni masqué.
        assert!(
            !json.contains("feather") && !json.contains("invert"),
            "{json}"
        );
        let back = DevelopSettings::from_json(&json).unwrap();
        assert_eq!(back.masks, s.masks);
        assert!(!back.is_neutral());
    }

    #[test]
    fn masques_bornes_et_formes_invalides_retirees() {
        let s = DevelopSettings::from_json(
            r#"{"version":2,"process":1,"masks":[
                {"shape":{"kind":"radial","center":[5,-3],"radius":[0,9],"angle":540,"feather":300},
                 "adjust":{"exposure":12,"saturation":-400}},
                {"shape":{"kind":"linear","from":[0.2,0.2],"to":[0.2,0.2]}},
                {"shape":{"kind":"linear","from":[0,0],"to":[1,1]},"hidden":true,
                 "adjust":{"contrast":30}}
            ]}"#,
        )
        .unwrap();
        // Le dégradé linéaire réduit à un point est retiré.
        assert_eq!(s.masks.len(), 2);
        let MaskShape::Radial {
            center,
            radius,
            angle,
            feather,
        } = s.masks[0].shape
        else {
            panic!("dégradé radial attendu");
        };
        assert_eq!(center, [2.0, -1.0]);
        assert_eq!(radius, [MASK_SIZE_MIN, 2.0]);
        assert_eq!((angle, feather), (-180.0, 100.0));
        assert_eq!(
            (s.masks[0].adjust.exposure, s.masks[0].adjust.saturation),
            (4.0, -100.0)
        );
        // Un masque masqué ne compte pas : sans le premier, la photo est neutre.
        let only_hidden = DevelopSettings {
            masks: vec![s.masks[1].clone()],
            ..DevelopSettings::default()
        };
        assert!(only_hidden.is_neutral());
    }

    #[test]
    fn pas_plus_de_masques_que_la_limite() {
        let s = DevelopSettings {
            masks: vec![degrade_radial(); MASKS_MAX + 5],
            ..DevelopSettings::default()
        };
        assert_eq!(s.sanitized().masks.len(), MASKS_MAX);
    }
}
