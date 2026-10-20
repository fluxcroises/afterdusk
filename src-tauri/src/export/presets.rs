//! Presets d'export (cahier des charges, 4.6) : les cinq fournis, modifiables,
//! et ceux de l'utilisateur, avec le filigrane et la dernière destination,
//! dans `export.json` à côté de la base (il survit à sa reconstruction).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ExportError;
use super::color::ColorSpace;
use super::encode::Format;
use super::metadata::MetadataPolicy;
use super::render::{Resize, Sharpen};
use super::watermark::Watermark;
use crate::import::copy::part_path;

/// Fichier des réglages d'export, à côté de la base.
pub const FILE: &str = "export.json";
const VERSION: u32 = 1;

/// Ce que produit un preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Output {
    /// Le fichier d'origine, tel quel, et son XMP.
    Original,
    Jpeg {
        quality: u8,
    },
    Png {
        sixteen: bool,
    },
    Tiff {
        sixteen: bool,
    },
    Avif {
        quality: u8,
    },
}

impl Output {
    /// Format d'encodage, ou rien pour une copie de l'original.
    pub fn format(self) -> Option<Format> {
        match self {
            Self::Original => None,
            Self::Jpeg { quality } => Some(Format::Jpeg { quality }),
            Self::Png { sixteen } => Some(Format::Png { sixteen }),
            Self::Tiff { sixteen } => Some(Format::Tiff { sixteen }),
            Self::Avif { quality } => Some(Format::Avif { quality }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreset {
    /// « originals », « full », « web », « instagram », « print », ou
    /// « user-N ».
    pub id: String,
    /// Nom donné par l'utilisateur ; les presets fournis sont nommés par
    /// l'interface.
    #[serde(default)]
    pub name: Option<String>,
    pub output: Output,
    #[serde(default)]
    pub resize: Resize,
    #[serde(default)]
    pub color: ColorSpace,
    #[serde(default)]
    pub sharpen: Sharpen,
    #[serde(default)]
    pub metadata: MetadataPolicy,
    /// Poser le filigrane des réglages d'export.
    #[serde(default)]
    pub watermark: bool,
    /// Modèle du nom des fichiers : {name}, {original}, {date}, {seq}.
    #[serde(default = "default_naming")]
    pub naming: String,
}

fn default_naming() -> String {
    "{name}".into()
}

/// Les cinq presets fournis.
pub fn builtin() -> Vec<ExportPreset> {
    let preset = |id: &str, output, resize, color, sharpen, metadata| ExportPreset {
        id: id.into(),
        name: None,
        output,
        resize,
        color,
        sharpen,
        metadata,
        watermark: false,
        naming: default_naming(),
    };
    vec![
        preset(
            "originals",
            Output::Original,
            Resize::Full,
            ColorSpace::Srgb,
            Sharpen::None,
            MetadataPolicy::All,
        ),
        preset(
            "full",
            Output::Jpeg { quality: 95 },
            Resize::Full,
            ColorSpace::Srgb,
            Sharpen::None,
            MetadataPolicy::All,
        ),
        preset(
            "web",
            Output::Jpeg { quality: 85 },
            Resize::LongSide(2048),
            ColorSpace::Srgb,
            Sharpen::Screen,
            MetadataPolicy::NoGps,
        ),
        preset(
            "instagram",
            Output::Jpeg { quality: 90 },
            Resize::Fit([1080, 1350]),
            ColorSpace::Srgb,
            Sharpen::Screen,
            MetadataPolicy::NoGpsSerial,
        ),
        preset(
            "print",
            Output::Tiff { sixteen: true },
            Resize::Full,
            ColorSpace::AdobeRgb,
            Sharpen::Paper,
            MetadataPolicy::All,
        ),
    ]
}

/// Réglages d'export de l'utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    pub version: u32,
    pub presets: Vec<ExportPreset>,
    #[serde(default)]
    pub watermark: Option<Watermark>,
    #[serde(default)]
    pub destination: Option<PathBuf>,
    #[serde(default = "yes")]
    pub open_folder: bool,
}

fn yes() -> bool {
    true
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            version: VERSION,
            presets: builtin(),
            watermark: None,
            destination: None,
            open_folder: true,
        }
    }
}

impl ExportSettings {
    /// Valeurs ramenées dans leurs plages ; les presets fournis absents sont
    /// rajoutés (une version plus récente peut en apporter).
    pub fn sanitized(mut self) -> Self {
        self.version = VERSION;
        for b in builtin() {
            if !self.presets.iter().any(|p| p.id == b.id) {
                self.presets.push(b);
            }
        }
        self.presets.retain(|p| !p.id.is_empty());
        for p in &mut self.presets {
            p.output = match p.output {
                Output::Jpeg { quality } => Output::Jpeg {
                    quality: quality.clamp(1, 100),
                },
                Output::Avif { quality } => Output::Avif {
                    quality: quality.clamp(1, 100),
                },
                other => other,
            };
            // L'AVIF reste en sRGB.
            if matches!(p.output, Output::Avif { .. }) {
                p.color = ColorSpace::Srgb;
            }
            if p.naming.trim().is_empty() {
                p.naming = default_naming();
            }
            if let Some(name) = &p.name
                && name.trim().is_empty()
            {
                p.name = None;
            }
        }
        self
    }
}

/// Lit les réglages d'export ; les valeurs par défaut s'il n'y en a pas.
pub fn load(path: &Path) -> Result<ExportSettings, ExportError> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let s: ExportSettings =
                serde_json::from_str(&text).map_err(|e| ExportError::Settings(e.to_string()))?;
            if s.version > VERSION {
                return Err(ExportError::Settings(format!(
                    "version {} plus récente que celle d'Afterdusk",
                    s.version
                )));
            }
            Ok(s.sanitized())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ExportSettings::default()),
        Err(e) => Err(e.into()),
    }
}

/// Enregistre les réglages d'export d'un coup (fichier temporaire renommé).
pub fn save(path: &Path, settings: ExportSettings) -> Result<ExportSettings, ExportError> {
    let settings = settings.sanitized();
    let text = serde_json::to_string_pretty(&settings)
        .map_err(|e| ExportError::Settings(e.to_string()))?;
    let tmp = part_path(path);
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(settings)
}

/// Nom de fichier d'après le modèle : {name} (nom actuel), {original} (nom
/// sur la carte), {date} (AAAA-MM-JJ de prise de vue), {seq} (numéro dans
/// l'export, sur trois chiffres). Caractères interdits remplacés.
pub fn file_stem(
    template: &str,
    name: &str,
    original: &str,
    date: Option<&str>,
    seq: usize,
) -> String {
    let stem = |s: &str| {
        Path::new(s)
            .file_stem()
            .map_or_else(|| s.to_owned(), |v| v.to_string_lossy().into_owned())
    };
    let date = date.and_then(|d| d.get(..10)).unwrap_or("0000-00-00");
    let out = template
        .replace("{name}", &stem(name))
        .replace("{original}", &stem(original))
        .replace("{date}", date)
        .replace("{seq}", &format!("{seq:03}"));
    let clean = clean_name(&out);
    if clean.is_empty() { stem(name) } else { clean }
}

/// Nom de fichier sans caractères interdits ni points ou espaces aux bords.
pub fn clean_name(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    clean.trim().trim_matches('.').to_owned()
}

/// Premier chemin libre : « nom.ext », puis « nom (2).ext »… Jamais
/// d'écrasement.
pub fn free_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let with = |s: &str| {
        if ext.is_empty() {
            dir.join(s)
        } else {
            dir.join(format!("{s}.{ext}"))
        }
    };
    let first = with(stem);
    if !first.exists() && !part_path(&first).exists() {
        return first;
    }
    (2..)
        .map(|n| with(&format!("{stem} ({n})")))
        .find(|p| !p.exists() && !part_path(p).exists())
        .unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_fournis_conformes_au_cahier_des_charges() {
        let p = builtin();
        let ids: Vec<&str> = p.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["originals", "full", "web", "instagram", "print"]);
        assert_eq!(p[1].output, Output::Jpeg { quality: 95 });
        assert_eq!(p[2].resize, Resize::LongSide(2048));
        assert_eq!(p[2].metadata, MetadataPolicy::NoGps);
        assert_eq!(p[3].resize, Resize::Fit([1080, 1350]));
        assert_eq!(p[3].metadata, MetadataPolicy::NoGpsSerial);
        assert_eq!(p[4].output, Output::Tiff { sixteen: true });
        assert_eq!(p[4].color, ColorSpace::AdobeRgb);
    }

    #[test]
    fn reglages_lus_enregistres_et_completes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        assert_eq!(load(&path).unwrap(), ExportSettings::default());
        let mut s = ExportSettings::default();
        s.presets.retain(|p| p.id != "print");
        s.presets[2].output = Output::Jpeg { quality: 250 };
        s.presets.push(ExportPreset {
            id: "user-1".into(),
            name: Some("Client".into()),
            output: Output::Avif { quality: 70 },
            color: ColorSpace::AdobeRgb,
            ..builtin()[2].clone()
        });
        let saved = save(&path, s).unwrap();
        let read = load(&path).unwrap();
        assert_eq!(read, saved);
        // Qualité bornée, AVIF en sRGB, preset fourni retiré : rajouté.
        assert_eq!(read.presets[2].output, Output::Jpeg { quality: 100 });
        let user = read.presets.iter().find(|p| p.id == "user-1").unwrap();
        assert_eq!(user.color, ColorSpace::Srgb);
        assert!(read.presets.iter().any(|p| p.id == "print"));
    }

    #[test]
    fn noms_de_fichiers() {
        let n = |t| {
            file_stem(
                t,
                "2018-03-13_0042.ARW",
                "DSC01234.ARW",
                Some("2018-03-13T14:05:09"),
                7,
            )
        };
        assert_eq!(n("{name}"), "2018-03-13_0042");
        assert_eq!(n("{date} {original} {seq}"), "2018-03-13 DSC01234 007");
        assert_eq!(n("Client: {name}?"), "Client_ 2018-03-13_0042_");
        assert_eq!(n("   "), "2018-03-13_0042");
        let dir = tempfile::tempdir().unwrap();
        let first = free_path(dir.path(), "a", "jpg");
        fs::write(&first, b"x").unwrap();
        assert_eq!(
            free_path(dir.path(), "a", "jpg").file_name().unwrap(),
            "a (2).jpg"
        );
    }
}
