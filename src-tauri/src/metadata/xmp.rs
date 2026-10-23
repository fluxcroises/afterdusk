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

/// Espace de noms des propriétés propres à Afterdusk, sur le domaine de
/// l'association qui héberge le projet (docs/decisions/0002-metadonnees-xmp.md).
pub const AFTERDUSK_NS: &str = "https://afterdusk.flux-croises.fr/ns/xmp/1.0/";
const AFTERDUSK_PREFIX: &str = "afterdusk";
/// Adresse provisoire écrite avant 2026-10-23, relue puis remplacée.
const LEGACY_NS: &str = "https://afterdusk.app/ns/xmp/1.0/";
const LEGACY_PREFIX: &str = "afterduskapp";
const OWN_PROPERTIES: [&str; 5] = ["Pick", "Develop", "Snapshots", "Versions", "LocationSource"];

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

impl Flag {
    /// Valeur de la colonne `media.flag` : 1 retenue, -1 rejetée, 0 sinon.
    pub fn as_db(self) -> i64 {
        match self {
            Self::None => 0,
            Self::Picked => 1,
            Self::Rejected => -1,
        }
    }

    pub fn from_db(value: i64) -> Self {
        match value {
            1 => Self::Picked,
            -1 => Self::Rejected,
            _ => Self::None,
        }
    }
}

impl ColorLabel {
    /// Valeur de la colonne `media.color`.
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        Self::from_xmp_label(value)
    }

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

/// Origine d'un lieu (colonne `media.location_source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocationSource {
    /// GPS écrit par le boîtier dans l'original.
    Exif,
    /// Trace GPX.
    Gpx,
    /// Photo de téléphone du même moment.
    Phone,
    /// Saisi par l'utilisateur.
    Manual,
}

impl LocationSource {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Exif => "exif",
            Self::Gpx => "gpx",
            Self::Phone => "phone",
            Self::Manual => "manual",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "exif" => Some(Self::Exif),
            "gpx" => Some(Self::Gpx),
            "phone" => Some(Self::Phone),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Lieu posé par Afterdusk : `exif:GPSLatitude` et `exif:GPSLongitude`
/// (format XMP « 48,51.504000N »), sa source en `afterdusk:LocationSource`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XmpLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub source: LocationSource,
}

/// Coordonnée au format XMP : degrés, minutes décimales et hémisphère.
fn xmp_coordinate(value: f64, positive: char, negative: char) -> String {
    let hemisphere = if value < 0.0 { negative } else { positive };
    let abs = value.abs();
    let degrees = abs.trunc();
    format!("{degrees},{:.6}{hemisphere}", (abs - degrees) * 60.0)
}

/// Lit « 48,51.504N » ou « 48,51,30N » ; rien si la valeur est illisible.
fn parse_coordinate(text: &str) -> Option<f64> {
    let text = text.trim();
    let hemisphere = text.chars().last()?.to_ascii_uppercase();
    let sign = match hemisphere {
        'N' | 'E' => 1.0,
        'S' | 'W' => -1.0,
        _ => return None,
    };
    let parts: Vec<f64> = text[..text.len() - 1]
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    let value = match parts.as_slice() {
        [d, m] => d + m / 60.0,
        [d, m, s] => d + m / 60.0 + s / 3600.0,
        _ => return None,
    };
    Some(sign * value)
}

/// Ce qu'Afterdusk écrit dans le XMP en V0.1.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
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
    /// Réglages de retouche (JSON versionné, décision 0005), `afterdusk:Develop`.
    /// `None` : photo jamais retouchée, la propriété est retirée.
    pub develop: Option<String>,
    /// Instantanés de la version originale (JSON), `afterdusk:Snapshots`.
    pub snapshots: Option<String>,
    /// Copies virtuelles, avec leurs réglages et instantanés (JSON),
    /// `afterdusk:Versions`.
    pub versions: Option<String>,
    /// Lieu posé après coup. `None` à l'écriture : le XMP garde le sien.
    pub location: Option<XmpLocation>,
    /// Heure de prise de vue corrigée, avec son fuseau
    /// (« 2026-07-14T12:00:00-04:00 »), en `exif:DateTimeOriginal`. `None` à
    /// l'écriture : le XMP garde la sienne.
    pub captured_at: Option<String>,
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
    let toolkit = |source| XmpError::Toolkit {
        path: path.to_path_buf(),
        source,
    };
    XmpMeta::register_namespace(AFTERDUSK_NS, AFTERDUSK_PREFIX).map_err(toolkit)?;
    XmpMeta::register_namespace(LEGACY_NS, LEGACY_PREFIX).map_err(toolkit)?;
    Ok(())
}

/// Passe les propriétés de l'ancienne adresse sous la nouvelle ; une valeur
/// déjà présente sous la nouvelle l'emporte.
fn migrate_legacy(meta: &mut XmpMeta) -> Result<(), xmp_toolkit::XmpError> {
    for name in OWN_PROPERTIES {
        let Some(old) = meta.property(LEGACY_NS, name) else {
            continue;
        };
        if !meta.contains_property(AFTERDUSK_NS, name) {
            meta.set_property(AFTERDUSK_NS, name, &old.value.as_str().into())?;
        }
        meta.delete_property(LEGACY_NS, name)?;
    }
    Ok(())
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
    let toolkit = |source| XmpError::Toolkit {
        path: path.to_path_buf(),
        source,
    };
    let mut meta = XmpMeta::from_str(&text).map_err(toolkit)?;
    migrate_legacy(&mut meta).map_err(toolkit)?;
    Ok(Some(meta))
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
        develop: meta.property(AFTERDUSK_NS, "Develop").map(|v| v.value),
        snapshots: meta.property(AFTERDUSK_NS, "Snapshots").map(|v| v.value),
        versions: meta.property(AFTERDUSK_NS, "Versions").map(|v| v.value),
        location: read_location(&meta),
        captured_at: meta
            .property(xmp_ns::EXIF, "DateTimeOriginal")
            .map(|v| v.value),
    }))
}

fn read_location(meta: &XmpMeta) -> Option<XmpLocation> {
    let latitude = parse_coordinate(&meta.property(xmp_ns::EXIF, "GPSLatitude")?.value)?;
    let longitude = parse_coordinate(&meta.property(xmp_ns::EXIF, "GPSLongitude")?.value)?;
    // Lieu écrit par un autre logiciel : posé par l'utilisateur.
    let source = meta
        .property(AFTERDUSK_NS, "LocationSource")
        .and_then(|v| LocationSource::from_db(&v.value))
        .unwrap_or(LocationSource::Manual);
    Some(XmpLocation {
        latitude,
        longitude,
        source,
    })
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

    for (name, value) in [
        ("Develop", &data.develop),
        ("Snapshots", &data.snapshots),
        ("Versions", &data.versions),
    ] {
        match value {
            Some(json) => meta.set_property(AFTERDUSK_NS, name, &json.as_str().into()),
            None => meta.delete_property(AFTERDUSK_NS, name),
        }
        .map_err(toolkit)?;
    }

    if let Some(loc) = &data.location {
        for (name, value) in [
            ("GPSLatitude", xmp_coordinate(loc.latitude, 'N', 'S')),
            ("GPSLongitude", xmp_coordinate(loc.longitude, 'E', 'W')),
            ("GPSVersionID", "2.2.0.0".to_owned()),
        ] {
            meta.set_property(xmp_ns::EXIF, name, &value.as_str().into())
                .map_err(toolkit)?;
        }
        meta.set_property(AFTERDUSK_NS, "LocationSource", &loc.source.as_db().into())
            .map_err(toolkit)?;
    }
    if let Some(at) = &data.captured_at {
        meta.set_property(xmp_ns::EXIF, "DateTimeOriginal", &at.as_str().into())
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
            develop: Some(r#"{"version":1,"process":1,"light":{"exposure":0.5}}"#.into()),
            snapshots: Some(r#"[{"name":"N&B","settings":{"version":1,"process":1}}]"#.into()),
            versions: None,
            location: None,
            captured_at: Some("2026-07-14T12:00:00-04:00".into()),
        };
        write_sidecar(&path, &data).unwrap();
        assert_eq!(read_sidecar(&path).unwrap(), Some(data));
    }

    #[test]
    fn lieu_aller_retour_et_lieu_d_un_autre_logiciel() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DSCF0002.xmp");
        // Cabourg, et un point de l'hémisphère sud-ouest.
        for (lat, lon) in [
            (49.2869, -0.1166),
            (-33.8568, 151.2153),
            (-22.9519, -43.2105),
        ] {
            let data = SidecarData {
                location: Some(XmpLocation {
                    latitude: lat,
                    longitude: lon,
                    source: LocationSource::Gpx,
                }),
                ..SidecarData::default()
            };
            write_sidecar(&path, &data).unwrap();
            let back = read_sidecar(&path).unwrap().unwrap().location.unwrap();
            assert!((back.latitude - lat).abs() < 1e-6, "{back:?}");
            assert!((back.longitude - lon).abs() < 1e-6, "{back:?}");
            assert_eq!(back.source, LocationSource::Gpx);
        }
        // Écrire sans lieu ne retire pas celui du XMP.
        write_sidecar(&path, &SidecarData::default()).unwrap();
        assert!(read_sidecar(&path).unwrap().unwrap().location.is_some());

        let other = dir.path().join("IMG_0002.xmp");
        fs::write(
            &other,
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:exif="http://ns.adobe.com/exif/1.0/"
    exif:GPSLatitude="48,51,30N" exif:GPSLongitude="2,17.4W"/>
 </rdf:RDF>
</x:xmpmeta>"#,
        )
        .unwrap();
        let loc = read_sidecar(&other).unwrap().unwrap().location.unwrap();
        assert!((loc.latitude - 48.858_333).abs() < 1e-5);
        assert!((loc.longitude + 2.29).abs() < 1e-9);
        assert_eq!(loc.source, LocationSource::Manual);
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

    #[test]
    fn reprend_l_ancienne_adresse_de_l_espace_de_noms() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DSCF0001.xmp");
        fs::write(
            &path,
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:afterdusk="https://afterdusk.app/ns/xmp/1.0/"
    xmp:Rating="3"
    afterdusk:Pick="1"
    afterdusk:Develop="{&quot;version&quot;:1}"/>
 </rdf:RDF>
</x:xmpmeta>"#,
        )
        .unwrap();

        let data = read_sidecar(&path).unwrap().unwrap();
        assert_eq!(data.flag, Flag::Picked);
        assert_eq!(data.develop.as_deref(), Some(r#"{"version":1}"#));

        write_sidecar(&path, &data).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("afterdusk.app"), "{text}");
        assert!(text.contains(AFTERDUSK_NS), "{text}");
        let back = read_sidecar(&path).unwrap().unwrap();
        assert_eq!(back.flag, Flag::Picked);
        assert_eq!(back.develop, data.develop);
    }
}
