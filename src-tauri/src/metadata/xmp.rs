//! Fichiers XMP annexes (sidecars) écrits à côté des originaux.
//!
//! L'original n'est jamais ouvert en écriture : seul le fichier `.xmp` est
//! créé ou mis à jour, de façon atomique (fichier temporaire puis
//! renommage). Les propriétés écrites par d'autres logiciels sont conservées.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use xmp_toolkit::{ToStringOptions, XmpMeta, XmpValue, xmp_ns};

/// Espace de noms des propriétés propres à Afterdusk (proposition, voir
/// docs/decisions/0001-metadonnees-xmp.md).
pub const AFTERDUSK_NS: &str = "https://afterdusk.app/ns/xmp/1.0/";
const AFTERDUSK_PREFIX: &str = "afterdusk";

#[derive(Debug, thiserror::Error)]
pub enum XmpError {
    #[error("le chemin {0} n'est pas un fichier .xmp")]
    NotASidecar(PathBuf),
    #[error("erreur d'entrée-sortie sur {path} : {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("XMP non valide dans {path} : {source}")]
    Toolkit {
        path: PathBuf,
        #[source]
        source: xmp_toolkit::XmpError,
    },
    #[error("note hors limites : {0}")]
    InvalidRating(u8),
}

/// Nommage du fichier annexe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarNaming {
    /// `IMG_0001.xmp` (Lightroom, Bridge, Capture One).
    ReplaceExtension,
    /// `IMG_0001.RAF.xmp` (darktable, digiKam).
    AppendExtension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Flag {
    #[default]
    None,
    Picked,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorLabel {
    Red,
    Yellow,
    Green,
    Blue,
    Purple,
}

impl ColorLabel {
    /// Libellés par défaut de Lightroom, repris par Bridge et darktable.
    fn xmp_label(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
        }
    }

    fn from_xmp_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "red" | "rouge" => Some(Self::Red),
            "yellow" | "jaune" => Some(Self::Yellow),
            "green" | "vert" => Some(Self::Green),
            "blue" | "bleu" => Some(Self::Blue),
            "purple" | "violet" => Some(Self::Purple),
            _ => None,
        }
    }
}

/// Ce qu'Afterdusk écrit dans le XMP en V0.1.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SidecarData {
    /// 0 à 5 étoiles.
    pub rating: u8,
    pub flag: Flag,
    pub label: Option<ColorLabel>,
    pub keywords: Vec<String>,
    pub creator: Option<String>,
    pub rights: Option<String>,
    /// Nom du fichier sur la carte avant renommage.
    pub original_file_name: Option<String>,
}

pub fn sidecar_path(original: &Path, naming: SidecarNaming) -> PathBuf {
    match naming {
        SidecarNaming::ReplaceExtension => original.with_extension("xmp"),
        SidecarNaming::AppendExtension => {
            let mut name = original.as_os_str().to_owned();
            name.push(".xmp");
            PathBuf::from(name)
        }
    }
}

fn ensure_sidecar(path: &Path) -> Result<(), XmpError> {
    let is_xmp = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xmp"));
    if is_xmp {
        Ok(())
    } else {
        Err(XmpError::NotASidecar(path.to_path_buf()))
    }
}

fn register_namespace(path: &Path) -> Result<(), XmpError> {
    XmpMeta::register_namespace(AFTERDUSK_NS, AFTERDUSK_PREFIX)
        .map(|_| ())
        .map_err(|source| XmpError::Toolkit {
            path: path.to_path_buf(),
            source,
        })
}

fn load(path: &Path) -> Result<Option<XmpMeta>, XmpError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(XmpError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    XmpMeta::from_str(&text)
        .map(Some)
        .map_err(|source| XmpError::Toolkit {
            path: path.to_path_buf(),
            source,
        })
}

pub fn read_sidecar(path: &Path) -> Result<Option<SidecarData>, XmpError> {
    ensure_sidecar(path)?;
    register_namespace(path)?;
    let Some(meta) = load(path)? else {
        return Ok(None);
    };

    let rating = meta
        .property_i32(xmp_ns::XMP, "Rating")
        .map(|v| v.value)
        .unwrap_or(0);
    let flag = match meta.property_i32(AFTERDUSK_NS, "Pick").map(|v| v.value) {
        Some(1) => Flag::Picked,
        Some(-1) => Flag::Rejected,
        // Bridge et darktable notent un rejet par xmp:Rating = -1.
        _ if rating < 0 => Flag::Rejected,
        _ => Flag::None,
    };
    let keywords = meta
        .property_array(xmp_ns::DC, "subject")
        .map(|v| v.value)
        .collect();
    let creator = meta
        .property_array(xmp_ns::DC, "creator")
        .next()
        .map(|v| v.value);
    let rights = meta
        .localized_text(xmp_ns::DC, "rights", None, "x-default")
        .map(|(v, _)| v.value);

    Ok(Some(SidecarData {
        rating: u8::try_from(rating.clamp(0, 5)).unwrap_or(0),
        flag,
        label: meta
            .property(xmp_ns::XMP, "Label")
            .and_then(|v| ColorLabel::from_xmp_label(&v.value)),
        keywords,
        creator,
        rights,
        original_file_name: meta
            .property(xmp_ns::XMP_MM, "PreservedFileName")
            .map(|v| v.value),
    }))
}

/// Crée ou met à jour le XMP annexe en conservant les propriétés inconnues.
pub fn write_sidecar(path: &Path, data: &SidecarData) -> Result<(), XmpError> {
    ensure_sidecar(path)?;
    if data.rating > 5 {
        return Err(XmpError::InvalidRating(data.rating));
    }
    register_namespace(path)?;
    let toolkit = |source| XmpError::Toolkit {
        path: path.to_path_buf(),
        source,
    };

    let mut meta = match load(path)? {
        Some(meta) => meta,
        None => XmpMeta::new().map_err(toolkit)?,
    };

    meta.set_property_i32(xmp_ns::XMP, "Rating", &i32::from(data.rating).into())
        .map_err(toolkit)?;

    match data.flag {
        Flag::None => meta.delete_property(AFTERDUSK_NS, "Pick"),
        Flag::Picked => meta.set_property_i32(AFTERDUSK_NS, "Pick", &1.into()),
        Flag::Rejected => meta.set_property_i32(AFTERDUSK_NS, "Pick", &(-1).into()),
    }
    .map_err(toolkit)?;

    match data.label {
        Some(label) => meta.set_property(xmp_ns::XMP, "Label", &label.xmp_label().into()),
        None => meta.delete_property(xmp_ns::XMP, "Label"),
    }
    .map_err(toolkit)?;

    meta.delete_property(xmp_ns::DC, "subject")
        .map_err(toolkit)?;
    let bag = XmpValue::from("subject").set_is_array(true);
    for keyword in &data.keywords {
        meta.append_array_item(xmp_ns::DC, &bag, &keyword.as_str().into())
            .map_err(toolkit)?;
    }

    if let Some(creator) = &data.creator {
        meta.delete_property(xmp_ns::DC, "creator")
            .map_err(toolkit)?;
        let seq = XmpValue::from("creator")
            .set_is_array(true)
            .set_is_ordered(true);
        meta.append_array_item(xmp_ns::DC, &seq, &creator.as_str().into())
            .map_err(toolkit)?;
    }
    if let Some(rights) = &data.rights {
        meta.set_localized_text(xmp_ns::DC, "rights", None, "x-default", rights)
            .map_err(toolkit)?;
    }
    if let Some(name) = &data.original_file_name {
        meta.set_property(xmp_ns::XMP_MM, "PreservedFileName", &name.as_str().into())
            .map_err(toolkit)?;
    }

    let text = meta
        .to_string_with_options(ToStringOptions::default().omit_packet_wrapper())
        .map_err(toolkit)?;
    write_atomically(path, text.as_bytes())
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), XmpError> {
    let io = |source| XmpError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);

    let mut file = fs::File::create(&tmp).map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.sync_all().map_err(io)?;
    drop(file);
    fs::rename(&tmp, path).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nommage_des_annexes() {
        let raw = Path::new("2026/2026-10-18 Sortie/DSCF0001.RAF");
        assert_eq!(
            sidecar_path(raw, SidecarNaming::ReplaceExtension),
            Path::new("2026/2026-10-18 Sortie/DSCF0001.xmp")
        );
        assert_eq!(
            sidecar_path(raw, SidecarNaming::AppendExtension),
            Path::new("2026/2026-10-18 Sortie/DSCF0001.RAF.xmp")
        );
    }

    #[test]
    fn refuse_d_ecrire_ailleurs_que_dans_un_xmp() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("DSCF0001.RAF");
        let err = write_sidecar(&original, &SidecarData::default()).unwrap_err();
        assert!(matches!(err, XmpError::NotASidecar(_)));
        assert!(!original.exists());
    }

    #[test]
    fn aller_retour() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DSCF0001.xmp");
        let data = SidecarData {
            rating: 4,
            flag: Flag::Picked,
            label: Some(ColorLabel::Green),
            keywords: vec!["coucher de soleil".into(), "Bretagne".into()],
            creator: Some("Lewis Bedar".into()),
            rights: Some("© 2026 Lewis Bedar".into()),
            original_file_name: Some("DSCF0001.RAF".into()),
        };
        write_sidecar(&path, &data).unwrap();
        assert_eq!(read_sidecar(&path).unwrap(), Some(data));
    }

    #[test]
    fn conserve_les_proprietes_des_autres_logiciels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("IMG_0001.xmp");
        fs::write(
            &path,
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
    xmp:Rating="2"
    crs:Exposure2012="+0.50"/>
 </rdf:RDF>
</x:xmpmeta>"#,
        )
        .unwrap();

        let mut data = read_sidecar(&path).unwrap().unwrap();
        assert_eq!(data.rating, 2);
        data.rating = 5;
        data.flag = Flag::Rejected;
        write_sidecar(&path, &data).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        let meta = XmpMeta::from_str(&text).unwrap();
        let crs = "http://ns.adobe.com/camera-raw-settings/1.0/";
        assert_eq!(
            meta.property(crs, "Exposure2012")
                .map(|v| v.value)
                .as_deref(),
            Some("+0.50"),
            "{text}"
        );
        assert!(!text.contains("xpacket"), "{text}");
        let back = read_sidecar(&path).unwrap().unwrap();
        assert_eq!(back.rating, 5);
        assert_eq!(back.flag, Flag::Rejected);
    }

    #[test]
    fn retire_le_drapeau_et_la_couleur() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.xmp");
        let mut data = SidecarData {
            flag: Flag::Picked,
            label: Some(ColorLabel::Red),
            ..SidecarData::default()
        };
        write_sidecar(&path, &data).unwrap();
        data.flag = Flag::None;
        data.label = None;
        write_sidecar(&path, &data).unwrap();
        let back = read_sidecar(&path).unwrap().unwrap();
        assert_eq!(back.flag, Flag::None);
        assert_eq!(back.label, None);
    }
}
