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

/// Lumière : en IL pour l'exposition, de -100 à +100 pour le reste.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Light {
    #[serde(skip_serializing_if = "is_zero")]
    pub exposure: f64,
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
}

impl Default for DevelopSettings {
    /// Rendu neutre, dernier processus : une photo jamais retouchée.
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            process: LATEST_PROCESS,
            light: Light::default(),
            color: Color::default(),
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
        self
    }

    pub fn is_neutral(&self) -> bool {
        self.light.is_default() && self.color.is_default()
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
