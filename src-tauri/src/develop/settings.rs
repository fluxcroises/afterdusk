//! Réglages de retouche (décision 0005) : un objet JSON creux, par sections,
//! avec `version` (forme du JSON, migrée à la lecture) et `process` (calcul du
//! rendu, figé pour une photo déjà retouchée).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Forme actuelle du JSON. Toute évolution ajoute une étape à [`migrate`].
pub const FORMAT_VERSION: u32 = 1;

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

    fn values_mut(&mut self) -> [&mut f64; 8] {
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
            effects: Effects::default(),
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
    // Version 1 : forme actuelle. Une future version 2 ajoutera ici
    // `if from < 2 { value = v1_vers_v2(value) }`, testée sur un JSON v1.
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

    /// JSON compact, sans les valeurs par défaut.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
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
        let v = &mut self.effects.vignette;
        v.amount = clamp(v.amount, -100.0, 100.0);
        v.midpoint = clamp(v.midpoint, 0.0, 100.0);
        v.feather = clamp(v.feather, 0.0, 100.0);
        let g = &mut self.effects.grain;
        g.amount = clamp(g.amount, 0.0, 100.0);
        g.size = clamp(g.size, 0.0, 100.0);
        g.roughness = clamp(g.roughness, 0.0, 100.0);
        self
    }

    pub fn is_neutral(&self) -> bool {
        self.light.is_default()
            && self.color.is_default()
            && self.curve.is_default()
            && self.hsl.is_default()
            && self.detail.is_default()
            && self.effects.is_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            DevelopSettings::from_json(r#"{"version":2,"process":1}"#),
            Err(SettingsError::TooNewFormat { found: 2, .. })
        ));
        assert!(matches!(
            DevelopSettings::from_json(r#"{"version":1,"process":9}"#),
            Err(SettingsError::TooNewProcess { found: 9, .. })
        ));
        // Sans processus noté : le plus récent ; sections inconnues ignorées.
        let s = DevelopSettings::from_json(r#"{"version":1,"light":{"shadows":20}}"#).unwrap();
        assert_eq!((s.process, s.light.shadows), (LATEST_PROCESS, 20.0));
    }
}
