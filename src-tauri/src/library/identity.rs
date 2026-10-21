//! Identité du photographe (cahier des charges §4.10) : nom, copyright et site,
//! écrits dans les XMP à l'import (`dc:creator`, `dc:rights`) et repris plus
//! tard par le filigrane. Réglage `identity`, en JSON.
//!
//! Logo du studio, à part (réglage `identity_logo`) : copié et vérifié dans
//! le dossier des données, repris par les planches contact, l'inventaire du
//! matériel et le filigrane. La source n'est jamais touchée.

use std::path::{Path, PathBuf};

use image::DynamicImage;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::LibraryError;

const KEY: &str = "identity";
const LOGO_KEY: &str = "identity_logo";
/// Formats acceptés pour le logo (le PNG garde sa transparence).
const LOGO_FORMATS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Identity {
    pub name: String,
    /// « © 2026 Prénom Nom » par exemple ; vide : aucun copyright écrit.
    pub copyright: String,
    pub website: String,
}

impl Identity {
    fn trimmed(self) -> Self {
        Self {
            name: self.name.trim().to_owned(),
            copyright: self.copyright.trim().to_owned(),
            website: self.website.trim().to_owned(),
        }
    }
}

pub fn get(conn: &Connection) -> Result<Identity, LibraryError> {
    let value = setting(conn, KEY)?;
    // Un réglage illisible ne bloque rien : identité vide.
    Ok(value
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default())
}

pub fn set(conn: &Connection, identity: Identity) -> Result<Identity, LibraryError> {
    let identity = identity.trimmed();
    let json = serde_json::to_string(&identity).unwrap_or_else(|_| "{}".to_owned());
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [KEY, json.as_str()],
    )?;
    Ok(identity)
}

fn setting(conn: &Connection, key: &str) -> Result<Option<String>, LibraryError> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

/// Fichier du logo, relatif au dossier des données, s'il y en a un.
pub fn logo_path(conn: &Connection, data_dir: &Path) -> Result<Option<PathBuf>, LibraryError> {
    Ok(setting(conn, LOGO_KEY)?
        .map(|rel| data_dir.join(rel))
        .filter(|p| p.is_file()))
}

/// Le logo décodé, prêt pour un PDF ou un filigrane.
pub fn logo(conn: &Connection, data_dir: &Path) -> Option<DynamicImage> {
    image::open(logo_path(conn, data_dir).ok()??).ok()
}

/// Remplace le logo par une copie vérifiée de `source` ; l'ancienne copie
/// est retirée.
pub fn set_logo(conn: &Connection, data_dir: &Path, source: &Path) -> Result<(), LibraryError> {
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| LOGO_FORMATS.contains(&e.as_str()))
        .ok_or_else(|| LibraryError::Logo("format non pris en charge".into()))?;
    image::open(source).map_err(|e| LibraryError::Logo(e.to_string()))?;
    let dir = data_dir.join("identity");
    std::fs::create_dir_all(&dir).map_err(|e| LibraryError::Logo(e.to_string()))?;
    let name = format!("logo-{}.{ext}", chrono::Utc::now().timestamp_millis());
    crate::import::copy::copy_verified(source, &dir.join(&name))
        .map_err(|e| LibraryError::Logo(e.to_string()))?;
    clear_logo(conn, data_dir)?;
    let rel = format!("identity/{name}");
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [LOGO_KEY, rel.as_str()],
    )?;
    Ok(())
}

/// Retire le logo : la copie, pas la source.
pub fn clear_logo(conn: &Connection, data_dir: &Path) -> Result<(), LibraryError> {
    if let Some(path) = logo_path(conn, data_dir)? {
        let _ = std::fs::remove_file(path);
    }
    conn.execute("DELETE FROM settings WHERE key = ?1", [LOGO_KEY])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identite_enregistree_et_relue() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(get(&conn).unwrap(), Identity::default());
        let saved = set(
            &conn,
            Identity {
                name: "  Lewis Bedar ".into(),
                copyright: "© 2026 Lewis Bedar".into(),
                website: String::new(),
            },
        )
        .unwrap();
        assert_eq!(saved.name, "Lewis Bedar");
        assert_eq!(get(&conn).unwrap(), saved);
    }

    #[test]
    fn logo_copie_remplace_puis_retire() {
        let conn = crate::db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("donnees");
        let source = dir.path().join("studio.png");
        image::RgbaImage::from_pixel(40, 20, image::Rgba([10, 20, 30, 128]))
            .save(&source)
            .unwrap();
        let before = std::fs::read(&source).unwrap();
        assert!(logo(&conn, &data).is_none());
        set_logo(&conn, &data, &source).unwrap();
        let first = logo_path(&conn, &data).unwrap().unwrap();
        let img = logo(&conn, &data).unwrap();
        assert_eq!((img.width(), img.height()), (40, 20));
        assert!(img.color().has_alpha(), "la transparence reste");
        // Un nouveau logo remplace l'ancien, dont la copie disparaît.
        set_logo(&conn, &data, &source).unwrap();
        assert!(!first.exists());
        clear_logo(&conn, &data).unwrap();
        assert!(logo(&conn, &data).is_none());
        assert_eq!(std::fs::read(&source).unwrap(), before, "source intacte");
        // Un texte n'est pas une image.
        let txt = dir.path().join("logo.png");
        std::fs::write(&txt, b"pas une image").unwrap();
        assert!(matches!(
            set_logo(&conn, &data, &txt),
            Err(LibraryError::Logo(_))
        ));
    }
}
