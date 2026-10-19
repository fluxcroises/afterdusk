//! Chemins stockés en base : relatifs à la racine de la bibliothèque,
//! séparateur `/`, Unicode NFC (macOS renvoie des noms décomposés).

use std::path::{Component, Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("{path} n'est pas dans la bibliothèque {root}")]
    OutsideLibrary { path: PathBuf, root: PathBuf },
    #[error("chemin non représentable en UTF-8 : {0}")]
    NotUtf8(PathBuf),
    #[error("chemin relatif non valide : {0}")]
    InvalidRelative(String),
}

/// Chemin relatif normalisé, prêt à être stocké.
pub fn to_relative(root: &Path, path: &Path) -> Result<String, PathError> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| PathError::OutsideLibrary {
            path: path.to_path_buf(),
            root: root.to_path_buf(),
        })?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(
                part.to_str()
                    .ok_or_else(|| PathError::NotUtf8(path.to_path_buf()))?
                    .nfc()
                    .collect::<String>(),
            ),
            _ => return Err(PathError::InvalidRelative(rel.display().to_string())),
        }
    }
    Ok(parts.join("/"))
}

/// Chemin absolu à partir d'un chemin relatif stocké, en refusant de sortir de la racine.
pub fn to_absolute(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            return Err(PathError::InvalidRelative(relative.to_owned()));
        }
        path.push(part);
    }
    Ok(path)
}

/// Normalise un texte en NFC (noms d'albums, de fichiers).
pub fn nfc(text: &str) -> String {
    text.nfc().collect()
}

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Rend un fragment de nom valide sur Windows et macOS : caractères interdits
/// remplacés, espaces et points de fin retirés, noms réservés évités.
pub fn sanitize_component(text: &str) -> String {
    let cleaned: String = nfc(text)
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']).to_owned();
    let stem = trimmed.split('.').next().unwrap_or_default();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        format!("_{trimmed}")
    } else {
        trimmed
    }
}

/// Le plus proche dossier existant du chemin (lui-même s'il existe).
fn existing_ancestor(path: &Path) -> Option<&Path> {
    path.ancestors().find(|p| p.exists())
}

/// Deux dossiers sont-ils sur le même disque ? Une sauvegarde sur le disque
/// de la bibliothèque ne protège pas d'une panne : l'interface le signale.
/// `None` si l'on ne peut pas le savoir (aucun dossier existant).
pub fn same_volume(a: &Path, b: &Path) -> Option<bool> {
    let a = existing_ancestor(a)?;
    let b = existing_ancestor(b)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(a.metadata().ok()?.dev() == b.metadata().ok()?.dev())
    }
    #[cfg(not(unix))]
    {
        // Lettre de lecteur ou partage réseau, sans tenir compte de la casse.
        let prefix = |p: &Path| -> Option<String> {
            let canonical = p.canonicalize().ok()?;
            match canonical.components().next()? {
                Component::Prefix(prefix) => {
                    Some(prefix.as_os_str().to_string_lossy().to_lowercase())
                }
                _ => None,
            }
        };
        Some(prefix(a)? == prefix(b)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meme_disque() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("Bibliotheque");
        let b = dir.path().join("Sauvegarde").join("pas encore créé");
        assert_eq!(same_volume(&a, &b), Some(true));
    }

    #[test]
    fn relatif_avec_barres_obliques_et_nfc() {
        let root = Path::new("bibliotheque");
        // « É » décomposé (E + accent), comme le renvoie macOS.
        let path = root
            .join("2026")
            .join("2026-10-18 E\u{301}te\u{301}")
            .join("a.RAF");
        let rel = to_relative(root, &path).unwrap();
        assert_eq!(rel, "2026/2026-10-18 \u{c9}t\u{e9}/a.RAF");
        assert_eq!(
            to_absolute(root, &rel).unwrap(),
            root.join("2026").join("2026-10-18 Été").join("a.RAF")
        );
    }

    #[test]
    fn refuse_de_sortir_de_la_bibliotheque() {
        let root = Path::new("bibliotheque");
        assert!(to_absolute(root, "../secret").is_err());
        assert!(to_absolute(root, "a//b").is_err());
        assert!(to_absolute(root, "C:/Windows").is_err());
        assert!(to_relative(root, Path::new("ailleurs/a.RAF")).is_err());
    }

    #[test]
    fn noms_valides_partout() {
        assert_eq!(
            sanitize_component("Sortie: plage / mer?"),
            "Sortie- plage - mer-"
        );
        assert_eq!(sanitize_component("Fin. "), "Fin");
        assert_eq!(sanitize_component("con"), "_con");
        assert_eq!(sanitize_component("Concert"), "Concert");
    }
}
