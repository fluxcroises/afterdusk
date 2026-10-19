//! Détection des cartes mémoire et des boîtiers branchés en stockage de masse
//! (cahier des charges §4.1) : tout volume dont la racine contient un dossier
//! `DCIM`. La détection ne fait que lire : rien n'est jamais écrit sur la carte.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Un support détecté.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Racine du volume (`E:\`, `/Volumes/SD`).
    pub root: PathBuf,
    /// Nom du volume : lettre de lecteur sous Windows, nom du volume ailleurs.
    pub volume: String,
    /// Marque devinée d'après les dossiers du DCIM (`100_FUJI` → `FUJI`).
    pub camera: Option<String>,
}

/// Que faire à l'insertion d'une carte (réglage `devices.onInsert`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertBehavior {
    /// Bandeau « Carte détectée · Importer » (par défaut).
    #[default]
    Propose,
    /// Ouvre directement Importer sur la carte.
    Open,
    Nothing,
}

/// Racines où une carte peut apparaître sur ce système.
pub fn candidate_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        win::local_drives()
    }
    #[cfg(not(windows))]
    {
        let mut parents = vec![PathBuf::from("/Volumes"), PathBuf::from("/media")];
        if let Ok(user) = std::env::var("USER") {
            parents.push(PathBuf::from("/media").join(&user));
            parents.push(PathBuf::from("/run/media").join(&user));
        }
        parents
            .iter()
            .filter_map(|p| std::fs::read_dir(p).ok())
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect()
    }
}

#[cfg(windows)]
mod win {
    use std::path::PathBuf;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(root: *const u16) -> u32;
    }

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_RAMDISK: u32 = 6;

    /// Lecteurs locaux présents (amovibles, fixes, virtuels). Les lecteurs
    /// réseau, qui peuvent bloquer plusieurs secondes quand le serveur ne
    /// répond pas, et les lecteurs optiques sont écartés. A: et B: aussi
    /// (disquettes).
    pub fn local_drives() -> Vec<PathBuf> {
        // SAFETY : fonctions de kernel32 sans état ; la chaîne passée à
        // GetDriveTypeW est terminée par un zéro et vit pendant l'appel.
        let mask = unsafe { GetLogicalDrives() };
        (2..26u8)
            .filter(|i| mask & (1 << i) != 0)
            .filter_map(|i| {
                let root = format!("{}:\\", (b'A' + i) as char);
                let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
                let kind = unsafe { GetDriveTypeW(wide.as_ptr()) };
                matches!(kind, DRIVE_REMOVABLE | DRIVE_FIXED | DRIVE_RAMDISK)
                    .then(|| PathBuf::from(root))
            })
            .collect()
    }
}

/// Marque devinée d'après les sous-dossiers du DCIM (norme DCF : trois
/// chiffres puis cinq caractères libres, souvent la marque).
fn camera_hint(dcim: &Path) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(dcim)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.iter().find_map(|name| {
        let rest = name.get(3..)?;
        if !name.get(..3)?.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let hint = rest.trim_matches(|c: char| c == '_' || c == '-' || c.is_whitespace());
        // MSDCF (Sony) et autres suffixes génériques n'apprennent rien.
        let generic = ["MSDCF", "MEDIA", "DCIM", "PHOTO", "IMAGE"];
        (!hint.is_empty() && !generic.contains(&hint.to_uppercase().as_str()))
            .then(|| hint.to_uppercase())
    })
}

fn volume_name(root: &Path) -> String {
    #[cfg(windows)]
    {
        root.to_string_lossy().trim_end_matches('\\').to_owned()
    }
    #[cfg(not(windows))]
    {
        root.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned())
    }
}

/// Supports présents parmi les racines : celles qui contiennent un `DCIM`.
pub fn detect(roots: &[PathBuf]) -> Vec<Device> {
    let mut found: Vec<Device> = roots
        .iter()
        .filter_map(|root| {
            let dcim = root.join("DCIM");
            dcim.is_dir().then(|| Device {
                root: root.clone(),
                volume: volume_name(root),
                camera: camera_hint(&dcim),
            })
        })
        .collect();
    found.sort();
    found
}

/// Supports apparus et disparus entre deux relevés.
pub fn diff(before: &[Device], now: &[Device]) -> (Vec<Device>, Vec<Device>) {
    let old: BTreeSet<&PathBuf> = before.iter().map(|d| &d.root).collect();
    let new: BTreeSet<&PathBuf> = now.iter().map(|d| &d.root).collect();
    let added = now
        .iter()
        .filter(|d| !old.contains(&d.root))
        .cloned()
        .collect();
    let removed = before
        .iter()
        .filter(|d| !new.contains(&d.root))
        .cloned()
        .collect();
    (added, removed)
}

const BEHAVIOR_KEY: &str = "devices.onInsert";

/// Comportement à l'insertion d'une carte ; « proposer » par défaut.
pub fn insert_behavior(conn: &rusqlite::Connection) -> rusqlite::Result<InsertBehavior> {
    use rusqlite::OptionalExtension;
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [BEHAVIOR_KEY],
            |r| r.get(0),
        )
        .optional()?;
    Ok(value
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default())
}

pub fn set_insert_behavior(
    conn: &rusqlite::Connection,
    behavior: InsertBehavior,
) -> rusqlite::Result<()> {
    let json = serde_json::to_string(&behavior).unwrap_or_else(|_| "\"propose\"".to_owned());
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [BEHAVIOR_KEY, json.as_str()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carte_reconnue_a_son_dcim() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("SD");
        let disk = dir.path().join("Disque");
        std::fs::create_dir_all(card.join("DCIM").join("100_FUJI")).unwrap();
        std::fs::create_dir_all(card.join("DCIM").join("101_FUJI")).unwrap();
        std::fs::create_dir_all(disk.join("Documents")).unwrap();
        let sony = dir.path().join("Sony");
        std::fs::create_dir_all(sony.join("DCIM").join("100MSDCF")).unwrap();

        let found = detect(&[card.clone(), disk, sony.clone()]);
        assert_eq!(found.len(), 2);
        let fuji = found.iter().find(|d| d.root == card).unwrap();
        assert_eq!(fuji.camera.as_deref(), Some("FUJI"));
        let sony = found.iter().find(|d| d.root == sony).unwrap();
        assert_eq!(sony.camera, None);
    }

    #[test]
    fn apparitions_et_disparitions() {
        let a = Device {
            root: "E:\\".into(),
            volume: "E:".into(),
            camera: None,
        };
        let b = Device {
            root: "F:\\".into(),
            volume: "F:".into(),
            camera: Some("CANON".into()),
        };
        let (added, removed) = diff(std::slice::from_ref(&a), std::slice::from_ref(&b));
        assert_eq!(added, [b]);
        assert_eq!(removed, [a]);
    }

    #[test]
    fn comportement_a_l_insertion() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(insert_behavior(&conn).unwrap(), InsertBehavior::Propose);
        set_insert_behavior(&conn, InsertBehavior::Open).unwrap();
        assert_eq!(insert_behavior(&conn).unwrap(), InsertBehavior::Open);
    }

    #[test]
    fn indice_de_marque() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["100CANON", "MISC"] {
            std::fs::create_dir_all(dir.path().join(name)).unwrap();
        }
        assert_eq!(camera_hint(dir.path()).as_deref(), Some("CANON"));
    }
}
