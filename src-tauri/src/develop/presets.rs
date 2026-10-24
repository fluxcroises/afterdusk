//! Presets : fournis (catalogue `presets.json`, simulations de film
//! comprises) et personnels (fichier `presets.json` du dossier de données de
//! l'application, qui survit à une reconstruction de la base).
//!
//! Un preset n'est qu'un jeu de réglages ordinaires et la liste des réglages
//! qu'il commande (`controls` : « light.exposure », « curve »…). L'appliquer
//! remplace ces réglages-là, et eux seuls, par les siens dosés à l'intensité
//! choisie ; tout le reste de la photo est gardé. Les presets d'aspect
//! (essentiels, noir et blanc, films) commandent tous les mêmes réglages :
//! en choisir un autre remplace le précédent, sans toucher à l'exposition, à
//! la balance des blancs ni à la géométrie.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::settings::{CurvePoint, DevelopSettings, HueSet, Look};

/// Catalogue fourni avec l'application.
const CATALOG: &str = include_str!("presets.json");
/// Fichier des presets personnels, à côté de la base.
pub const USER_FILE: &str = "presets.json";
/// Forme du fichier des presets personnels.
const USER_VERSION: u32 = 1;
/// Préfixe des identifiants des presets personnels.
const USER_PREFIX: &str = "user-";
/// Longueur d'un nom de preset, au plus.
const NAME_MAX: usize = 80;

/// Sections et réglages qu'un preset peut commander.
const PATHS: &[&str] = &[
    "light",
    "light.exposure",
    "light.brightness",
    "light.contrast",
    "light.highlights",
    "light.shadows",
    "light.whites",
    "light.blacks",
    "color",
    "color.whiteBalance",
    "color.vibrance",
    "color.saturation",
    "curve",
    "hsl",
    "detail",
    "detail.sharpening",
    "detail.noise",
    "effects",
    "effects.vignette",
    "effects.grain",
    "lens",
];

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("catalogue de presets illisible : {0}")]
    Catalog(String),
    #[error("preset inconnu : {0}")]
    Unknown(String),
    #[error("nom de preset déjà pris : {0}")]
    NameTaken(String),
    #[error("nom de preset vide ou trop long")]
    InvalidName,
    #[error("réglage commandé inconnu : {0}")]
    InvalidControl(String),
    #[error("fichier des presets personnels illisible : {0}")]
    UserFile(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Pellicule évoquée par une simulation de film, pour dessiner sa boîte (aucun
/// nom ni logo de marque).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Film {
    /// `negative`, `slide` ou `bw`.
    pub kind: String,
    pub iso: u32,
    /// Couleurs de la boîte : dominante, puis secondaire.
    pub colors: [String; 2],
}

/// Un preset, fourni ou personnel.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: String,
    /// Nom donné par l'utilisateur ; les presets fournis sont nommés par
    /// l'interface d'après leur identifiant.
    pub name: Option<String>,
    pub settings: DevelopSettings,
    pub controls: Vec<String>,
    pub film: Option<Film>,
}

/// Résumé pour l'interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetSummary {
    pub id: String,
    pub name: Option<String>,
    pub film: Option<Film>,
    pub controls: Vec<String>,
}

/// Groupe de presets ; `user` : celui des presets personnels.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetGroup {
    pub id: String,
    pub user: bool,
    pub presets: Vec<PresetSummary>,
}

impl Preset {
    fn summary(&self) -> PresetSummary {
        PresetSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            film: self.film.clone(),
            controls: self.controls.clone(),
        }
    }
}

// Catalogue des presets intégrés.

#[derive(Deserialize)]
struct RawCatalog {
    looks: Vec<String>,
    groups: Vec<RawGroup>,
}

#[derive(Deserialize)]
struct RawGroup {
    id: String,
    /// « looks », ou une liste de réglages.
    controls: Value,
    presets: Vec<RawPreset>,
}

#[derive(Deserialize)]
struct RawPreset {
    id: String,
    settings: Value,
    #[serde(default)]
    film: Option<Film>,
}

/// Ce que le copier-coller peut transmettre en plus : géométrie et
/// corrections de défauts (jamais dans un preset).
const PASTE_ONLY: &[&str] = &["geometry", "spots", "masks"];

fn check_controls(controls: &[String]) -> Result<(), PresetError> {
    match controls.iter().find(|c| !PATHS.contains(&c.as_str())) {
        Some(bad) => Err(PresetError::InvalidControl(bad.clone())),
        None => Ok(()),
    }
}

/// Groupes du catalogue : identifiant et presets.
type Groups = Vec<(String, Vec<Preset>)>;

fn parse_catalog() -> Result<Groups, PresetError> {
    let raw: RawCatalog =
        serde_json::from_str(CATALOG).map_err(|e| PresetError::Catalog(e.to_string()))?;
    check_controls(&raw.looks)?;
    let mut groups = Vec::new();
    for g in raw.groups {
        let controls: Vec<String> = match &g.controls {
            Value::String(s) if s == "looks" => raw.looks.clone(),
            other => serde_json::from_value(other.clone())
                .map_err(|e| PresetError::Catalog(format!("{} : {e}", g.id)))?,
        };
        check_controls(&controls)?;
        let mut presets = Vec::new();
        for p in g.presets {
            let settings = DevelopSettings::from_json(&p.settings.to_string())
                .map_err(|e| PresetError::Catalog(format!("{} : {e}", p.id)))?;
            presets.push(Preset {
                id: p.id,
                name: None,
                settings,
                controls: controls.clone(),
                film: p.film,
            });
        }
        groups.push((g.id, presets));
    }
    Ok(groups)
}

/// Catalogue fourni, lu une fois.
fn catalog() -> Result<&'static [(String, Vec<Preset>)], PresetError> {
    static CACHE: OnceLock<Result<Groups, String>> = OnceLock::new();
    CACHE
        .get_or_init(|| parse_catalog().map_err(|e| e.to_string()))
        .as_deref()
        .map_err(|e| PresetError::Catalog(e.clone()))
}

// Presets personnels, rangés dans presets.json à côté de la base.

#[derive(Debug, Default, Serialize, Deserialize)]
struct UserFile {
    version: u32,
    presets: Vec<UserPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserPreset {
    id: String,
    name: String,
    settings: Value,
    controls: Vec<String>,
}

/// Une seule écriture du fichier à la fois.
static USER_LOCK: Mutex<()> = Mutex::new(());

fn read_user(path: &Path) -> Result<UserFile, PresetError> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let file: UserFile =
                serde_json::from_str(&text).map_err(|e| PresetError::UserFile(e.to_string()))?;
            if file.version > USER_VERSION {
                return Err(PresetError::UserFile(format!(
                    "version {} plus récente que celle d'Afterdusk",
                    file.version
                )));
            }
            Ok(file)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(UserFile {
            version: USER_VERSION,
            presets: Vec::new(),
        }),
        Err(e) => Err(e.into()),
    }
}

/// Écrit le fichier d'un coup : fichier temporaire, puis renommage.
fn write_user(path: &Path, file: &UserFile) -> Result<(), PresetError> {
    let text =
        serde_json::to_string_pretty(file).map_err(|e| PresetError::UserFile(e.to_string()))?;
    let tmp: PathBuf = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn user_presets(path: &Path) -> Result<Vec<Preset>, PresetError> {
    read_user(path)?
        .presets
        .into_iter()
        .map(|p| {
            Ok(Preset {
                settings: DevelopSettings::from_json(&p.settings.to_string())
                    .map_err(|e| PresetError::UserFile(e.to_string()))?,
                id: p.id,
                name: Some(p.name),
                controls: p.controls,
                film: None,
            })
        })
        .collect()
}

fn valid_name(name: &str) -> Result<String, PresetError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(PresetError::InvalidName);
    }
    Ok(name.to_owned())
}

/// Nouveau preset personnel, depuis les réglages d'une photo : seuls les
/// réglages `controls` y sont gardés.
pub fn create(
    path: &Path,
    name: &str,
    settings: &DevelopSettings,
    controls: Vec<String>,
) -> Result<PresetSummary, PresetError> {
    check_controls(&controls)?;
    let name = valid_name(name)?;
    let _guard = USER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read_user(path)?;
    if file
        .presets
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&name))
    {
        return Err(PresetError::NameTaken(name));
    }
    let next = file
        .presets
        .iter()
        .filter_map(|p| p.id.strip_prefix(USER_PREFIX)?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    // Seuls les réglages commandés sont enregistrés.
    let kept = apply_paths(&DevelopSettings::default(), settings, &controls);
    let preset = UserPreset {
        id: format!("{USER_PREFIX}{next}"),
        name,
        settings: serde_json::from_str(&kept.to_json())
            .map_err(|e| PresetError::UserFile(e.to_string()))?,
        controls,
    };
    file.version = USER_VERSION;
    file.presets.push(preset.clone());
    write_user(path, &file)?;
    Ok(PresetSummary {
        id: preset.id,
        name: Some(preset.name),
        film: None,
        controls: preset.controls,
    })
}

pub fn rename(path: &Path, id: &str, name: &str) -> Result<(), PresetError> {
    let name = valid_name(name)?;
    let _guard = USER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read_user(path)?;
    if file
        .presets
        .iter()
        .any(|p| p.id != id && p.name.eq_ignore_ascii_case(&name))
    {
        return Err(PresetError::NameTaken(name));
    }
    let preset = file
        .presets
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| PresetError::Unknown(id.to_owned()))?;
    preset.name = name;
    write_user(path, &file)
}

pub fn delete(path: &Path, id: &str) -> Result<(), PresetError> {
    let _guard = USER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read_user(path)?;
    let before = file.presets.len();
    file.presets.retain(|p| p.id != id);
    if file.presets.len() == before {
        return Err(PresetError::Unknown(id.to_owned()));
    }
    write_user(path, &file)
}

/// Groupes de presets : ceux du catalogue, puis les presets personnels.
pub fn list(path: &Path) -> Result<Vec<PresetGroup>, PresetError> {
    let mut groups: Vec<PresetGroup> = catalog()?
        .iter()
        .map(|(id, presets)| PresetGroup {
            id: id.clone(),
            user: false,
            presets: presets.iter().map(Preset::summary).collect(),
        })
        .collect();
    groups.push(PresetGroup {
        id: "user".into(),
        user: true,
        presets: user_presets(path)?.iter().map(Preset::summary).collect(),
    });
    Ok(groups)
}

/// Le preset `id`, fourni ou personnel.
pub fn find(path: &Path, id: &str) -> Result<Preset, PresetError> {
    if id.starts_with(USER_PREFIX) {
        return user_presets(path)?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| PresetError::Unknown(id.to_owned()));
    }
    catalog()?
        .iter()
        .flat_map(|(_, presets)| presets)
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| PresetError::Unknown(id.to_owned()))
}

// Intensité : rapproche chaque réglage du neutre.

fn lerp(neutral: f64, value: f64, k: f64) -> f64 {
    neutral + k * (value - neutral)
}

fn scale_hues(set: &mut HueSet, k: f64) {
    for v in set.values_mut() {
        *v *= k;
    }
}

fn scale_curve(points: &mut [CurvePoint], k: f64) {
    for p in points {
        p[1] = lerp(p[0], p[1], k);
    }
}

/// Réglages du preset à l'intensité `k` (0 à 1) : chaque valeur va de sa
/// valeur neutre à celle du preset. Les réglages de forme (taille du grain,
/// rayon de netteté, point milieu du vignetage…) et la balance des blancs
/// restent ceux du preset.
pub fn scaled(settings: &DevelopSettings, k: f64) -> DevelopSettings {
    let k = k.clamp(0.0, 1.0);
    let mut s = settings.clone();
    let l = &mut s.light;
    for v in [
        &mut l.exposure,
        &mut l.brightness,
        &mut l.contrast,
        &mut l.highlights,
        &mut l.shadows,
        &mut l.whites,
        &mut l.blacks,
    ] {
        *v *= k;
    }
    s.color.vibrance *= k;
    s.color.saturation *= k;
    for points in [
        &mut s.curve.rgb,
        &mut s.curve.red,
        &mut s.curve.green,
        &mut s.curve.blue,
    ] {
        scale_curve(points, k);
    }
    for set in [&mut s.hsl.hue, &mut s.hsl.saturation, &mut s.hsl.luminance] {
        scale_hues(set, k);
    }
    s.detail.sharpening.amount *= k;
    s.detail.noise.luminance *= k;
    s.detail.noise.color *= k;
    s.effects.vignette.amount *= k;
    s.effects.grain.amount *= k;
    let neutral = super::settings::LensSettings::default();
    s.lens.distortion = lerp(neutral.distortion, s.lens.distortion, k);
    s.lens.vignetting = lerp(neutral.vignetting, s.lens.vignetting, k);
    s.sanitized()
}

fn get_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, key| v.get(key))
}

/// Pose (ou retire, pour une valeur par défaut) `path` dans `v`, en retirant
/// les sections devenues vides.
fn set_path(v: &mut Value, path: &str, value: Option<&Value>) {
    let keys: Vec<&str> = path.split('.').collect();
    fn walk(v: &mut Value, keys: &[&str], value: Option<&Value>) {
        let Some(obj) = v.as_object_mut() else {
            return;
        };
        match keys {
            [] => {}
            [last] => match value {
                Some(x) => {
                    obj.insert((*last).to_owned(), x.clone());
                }
                None => {
                    obj.remove(*last);
                }
            },
            [first, rest @ ..] => {
                if value.is_some() && !obj.contains_key(*first) {
                    obj.insert((*first).to_owned(), Value::Object(Map::new()));
                }
                if let Some(child) = obj.get_mut(*first) {
                    walk(child, rest, value);
                    if child.as_object().is_some_and(Map::is_empty) {
                        obj.remove(*first);
                    }
                }
            }
        }
    }
    walk(v, &keys, value);
}

/// `current` dont les réglages `controls` prennent les valeurs de `from`.
fn apply_paths(
    current: &DevelopSettings,
    from: &DevelopSettings,
    controls: &[String],
) -> DevelopSettings {
    let mut cur: Value = serde_json::from_str(&current.to_json()).unwrap_or(Value::Null);
    let src: Value = serde_json::from_str(&from.to_json()).unwrap_or(Value::Null);
    // Une section entière avant ses réglages : l'ordre ne change rien.
    let ordered: BTreeSet<&String> = controls.iter().collect();
    for path in ordered {
        set_path(&mut cur, path, get_path(&src, path));
    }
    DevelopSettings::from_json(&cur.to_string()).unwrap_or_else(|_| current.clone())
}

/// `current` avec le preset appliqué à `amount` (0 à 100 %), noté dans `look`.
pub fn apply(current: &DevelopSettings, preset: &Preset, amount: f64) -> DevelopSettings {
    let amount = if amount.is_finite() {
        amount.clamp(0.0, 100.0)
    } else {
        100.0
    };
    let mut out = apply_paths(
        current,
        &scaled(&preset.settings, amount / 100.0),
        &preset.controls,
    );
    out.look = Some(Look {
        id: preset.id.clone(),
        amount,
    });
    out
}

/// Copier-coller : `current` dont les réglages `controls` prennent les valeurs
/// de `from`. L'aspect de `from` (galerie) le suit quand tout ce qu'il
/// commande est collé ; sinon celui de `current` reste noté.
pub fn paste(
    path: &Path,
    current: &DevelopSettings,
    from: &DevelopSettings,
    controls: &[String],
) -> Result<DevelopSettings, PresetError> {
    if let Some(bad) = controls
        .iter()
        .find(|c| !PATHS.contains(&c.as_str()) && !PASTE_ONLY.contains(&c.as_str()))
    {
        return Err(PresetError::InvalidControl(bad.clone()));
    }
    let mut out = apply_paths(current, from, controls);
    out.look = current.look.clone();
    if let Some(look) = &from.look {
        let covered = find(path, &look.id).is_ok_and(|p| {
            p.controls.iter().all(|c| {
                controls
                    .iter()
                    .any(|k| k == c || c.starts_with(&format!("{k}.")))
            })
        });
        if covered {
            out.look = Some(look.clone());
        }
    }
    Ok(out)
}

/// Retire l'aspect appliqué : les réglages qu'il commandait reviennent à leur
/// valeur neutre (vignette « Original » de la galerie).
pub fn remove(current: &DevelopSettings, preset: Option<&Preset>) -> DevelopSettings {
    let mut out = match preset {
        Some(p) => apply_paths(current, &DevelopSettings::default(), &p.controls),
        None => current.clone(),
    };
    out.look = None;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_complet_et_valide() {
        let groups = catalog().unwrap();
        let ids: Vec<&str> = groups.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["essentials", "bw", "film", "exposure", "vignette"]);
        let all: Vec<&Preset> = groups.iter().flat_map(|(_, p)| p).collect();
        assert!(all.len() >= 25);
        // Identifiants uniques ; chaque film a sa boîte.
        let unique: BTreeSet<&str> = all.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(unique.len(), all.len());
        for p in &groups[2].1 {
            assert!(p.film.is_some(), "{}", p.id);
        }
        // Aucun preset d'aspect ne touche à l'exposition ni à la balance.
        let looks = &groups[0].1[0].controls;
        for forbidden in ["light", "light.exposure", "color", "color.whiteBalance"] {
            assert!(!looks.iter().any(|c| c == forbidden), "{forbidden}");
        }
    }

    #[test]
    fn appliquer_garde_le_reste_et_dose_l_intensite() {
        let mut photo = DevelopSettings::default();
        photo.light.exposure = 0.7;
        photo.light.contrast = 12.0;
        photo.geometry.angle = 2.0;
        photo.curve.rgb = vec![[0.0, 0.0], [128.0, 150.0], [255.0, 255.0]];
        let punchy = find(Path::new("absent.json"), "punchy").unwrap();
        let full = apply(&photo, &punchy, 100.0);
        assert_eq!(full.light.exposure, 0.7);
        assert_eq!(full.geometry.angle, 2.0);
        assert_eq!(full.light.contrast, 35.0);
        assert_eq!(full.light.blacks, -15.0);
        // La courbe de la photo est commandée par l'aspect : retirée.
        assert!(full.curve.rgb.is_empty());
        assert_eq!(full.look.as_ref().map(|l| l.id.as_str()), Some("punchy"));
        let half = apply(&photo, &punchy, 50.0);
        assert_eq!(half.light.contrast, 17.5);
        assert_eq!(half.look.as_ref().map(|l| l.amount), Some(50.0));
        // Un autre aspect remplace le premier.
        let matte = find(Path::new("absent.json"), "matte").unwrap();
        let then = apply(&full, &matte, 100.0);
        assert_eq!(then.light.contrast, -5.0);
        assert_eq!(then.light.blacks, 0.0);
        assert_eq!(then.curve.rgb[0], [0.0, 30.0]);
        // L'exposition, elle, ne commande que l'exposition.
        let plus = find(Path::new("absent.json"), "exposurePlus1").unwrap();
        let both = apply(&then, &plus, 50.0);
        assert_eq!(both.light.exposure, 0.5);
        assert_eq!(both.curve.rgb, then.curve.rgb);
        // « Original » : les réglages de l'aspect reviennent au neutre.
        let back = remove(&then, Some(&matte));
        assert!(back.curve.rgb.is_empty() && back.look.is_none());
        assert_eq!((back.light.exposure, back.geometry.angle), (0.7, 2.0));
    }

    #[test]
    fn courbe_et_film_doses() {
        let film = find(Path::new("absent.json"), "filmBw400").unwrap();
        let s = scaled(&film.settings, 0.5);
        assert_eq!(s.color.saturation, -50.0);
        assert_eq!(s.effects.grain.amount, 17.5);
        // La taille du grain est une forme : gardée.
        assert_eq!(s.effects.grain.size, 35.0);
        let matte = find(Path::new("absent.json"), "matte").unwrap();
        let m = scaled(&matte.settings, 0.5);
        assert_eq!(m.curve.rgb[0], [0.0, 15.0]);
        assert_eq!(m.curve.rgb[3], [255.0, 247.5]);
        // À 0 % : rien.
        let zero = scaled(&film.settings, 0.0);
        assert!(zero.light.is_default() && zero.color.is_default());
    }

    #[test]
    fn copier_coller_choisi() {
        let none = Path::new("absent.json");
        let mut from = DevelopSettings::default();
        from.light.exposure = 1.0;
        from.light.contrast = 30.0;
        from.geometry.angle = 4.0;
        from.spots.push(crate::develop::settings::Spot {
            x: 0.5,
            y: 0.5,
            radius: 0.01,
            source_x: 0.6,
            source_y: 0.5,
            feather: 50.0,
            mode: crate::develop::settings::SpotMode::Heal,
        });
        from.masks.push(crate::develop::settings::Mask {
            shape: crate::develop::settings::MaskShape::Linear {
                from: [0.5, 0.0],
                to: [0.5, 0.4],
            },
            adjust: crate::develop::settings::LocalAdjust {
                exposure: -1.0,
                ..Default::default()
            },
            invert: false,
            hidden: false,
        });
        let mut target = DevelopSettings::default();
        target.light.exposure = -0.5;
        target.color.vibrance = 20.0;
        let c = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        // Masques collés : ceux de la photo copiée remplacent ceux de la cible.
        let masked = paste(none, &target, &from, &c(&["masks"])).unwrap();
        assert_eq!(masked.masks, from.masks);
        assert_eq!(masked.light.exposure, -0.5);
        let cleared = paste(none, &masked, &DevelopSettings::default(), &c(&["masks"])).unwrap();
        assert!(cleared.masks.is_empty());
        let out = paste(none, &target, &from, &c(&["light.contrast", "spots"])).unwrap();
        assert_eq!((out.light.exposure, out.light.contrast), (-0.5, 30.0));
        assert_eq!(out.color.vibrance, 20.0);
        assert_eq!(out.spots.len(), 1);
        assert_eq!(out.geometry.angle, 0.0);
        assert!(out.masks.is_empty());
        let all = paste(none, &target, &from, &c(&["light", "geometry"])).unwrap();
        assert_eq!((all.light.exposure, all.geometry.angle), (1.0, 4.0));
        assert!(paste(none, &target, &from, &c(&["version"])).is_err());
        // L'aspect suit quand tout ce qu'il commande est collé.
        let vivid = find(none, "vivid").unwrap();
        let styled = apply(&from, &vivid, 80.0);
        let looks = vivid.controls.clone();
        let with_look = paste(none, &target, &styled, &looks).unwrap();
        assert_eq!(with_look.look, styled.look);
        let partial = paste(none, &target, &styled, &c(&["curve"])).unwrap();
        assert!(partial.look.is_none());
        // Une section entière couvre ses réglages.
        let whole = paste(
            none,
            &target,
            &styled,
            &c(&["light", "color", "curve", "hsl", "effects"]),
        );
        assert_eq!(whole.unwrap().look, styled.look);
    }

    #[test]
    fn presets_personnels_dans_un_fichier() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(USER_FILE);
        let mut photo = DevelopSettings::default();
        photo.light.exposure = 0.3;
        photo.light.contrast = 20.0;
        photo.color.vibrance = 25.0;
        photo.geometry.angle = 3.0;
        let controls = vec!["light.contrast".to_owned(), "color".to_owned()];
        let created = create(&path, "  Mes couleurs ", &photo, controls).unwrap();
        assert_eq!(created.id, "user-1");
        assert_eq!(created.name.as_deref(), Some("Mes couleurs"));
        assert!(matches!(
            create(&path, "mes couleurs", &photo, vec![]),
            Err(PresetError::NameTaken(_))
        ));
        assert!(matches!(
            create(&path, "x", &photo, vec!["geometry".into()]),
            Err(PresetError::InvalidControl(_))
        ));
        // Seuls les réglages commandés sont enregistrés.
        let p = find(&path, "user-1").unwrap();
        assert_eq!(p.settings.light.exposure, 0.0);
        assert_eq!(p.settings.light.contrast, 20.0);
        assert_eq!(p.settings.geometry.angle, 0.0);
        let applied = apply(&DevelopSettings::default(), &p, 100.0);
        assert_eq!(
            (applied.light.contrast, applied.color.vibrance),
            (20.0, 25.0)
        );
        rename(&path, "user-1", "Couleurs").unwrap();
        let groups = list(&path).unwrap();
        let user = groups.last().unwrap();
        assert!(user.user);
        assert_eq!(user.presets[0].name.as_deref(), Some("Couleurs"));
        create(&path, "Deuxième", &photo, vec!["hsl".into()]).unwrap();
        delete(&path, "user-1").unwrap();
        assert!(find(&path, "user-1").is_err());
        assert_eq!(
            find(&path, "user-2").unwrap().name.as_deref(),
            Some("Deuxième")
        );
    }
}
