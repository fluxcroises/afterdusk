//! Copie vérifiée : un fichier n'est considéré comme copié qu'une fois relu
//! depuis la destination et sa somme BLAKE3 comparée à celle de la source.
//!
//! La copie passe par un fichier temporaire `.afterdusk-part` renommé à la
//! fin : une coupure ne laisse jamais un fichier tronqué sous le nom final.
//! La source est ouverte en lecture seule.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const BUFFER: usize = 1024 * 1024;
pub const PART_SUFFIX: &str = ".afterdusk-part";

#[derive(Debug, thiserror::Error)]
pub enum CopyError {
    #[error("lecture impossible de {path} : {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("écriture impossible de {path} : {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("la destination {0} existe déjà")]
    TargetExists(PathBuf),
    #[error("la copie de {path} ne correspond pas à l'original (somme de contrôle différente)")]
    Mismatch { path: PathBuf },
}

/// Somme BLAKE3 complète d'un fichier.
pub fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; BUFFER];
    loop {
        match file.read(&mut buf)? {
            0 => break,
            n => {
                hasher.update(&buf[..n]);
            }
        }
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn part_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(PART_SUFFIX);
    PathBuf::from(name)
}

/// Copie `source` vers `target` (qui ne doit pas exister) et renvoie la somme
/// BLAKE3 vérifiée. La date de modification de la source est conservée.
pub fn copy_verified(source: &Path, target: &Path) -> Result<String, CopyError> {
    let read_err = |source_err| CopyError::Read {
        path: source.to_path_buf(),
        source: source_err,
    };
    let write_err = |path: &Path| {
        let path = path.to_path_buf();
        move |source| CopyError::Write { path, source }
    };

    if target.exists() {
        return Err(CopyError::TargetExists(target.to_path_buf()));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(write_err(parent))?;
    }
    let part = part_path(target);
    // Reste d'une copie interrompue : fichier temporaire d'Afterdusk, jamais un original.
    if part.exists() {
        fs::remove_file(&part).map_err(write_err(&part))?;
    }

    let mut input = File::open(source).map_err(read_err)?;
    let modified = input.metadata().and_then(|m| m.modified()).ok();
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)
        .map_err(write_err(&part))?;

    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; BUFFER];
    loop {
        let n = input.read(&mut buf).map_err(read_err)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        output.write_all(&buf[..n]).map_err(write_err(&part))?;
    }
    output.sync_all().map_err(write_err(&part))?;
    if let Some(time) = modified {
        output.set_modified(time).map_err(write_err(&part))?;
    }
    drop(output);
    let expected = hasher.finalize().to_hex().to_string();

    // Relecture depuis la destination.
    let written = hash_file(&part).map_err(write_err(&part))?;
    if written != expected {
        let _ = fs::remove_file(&part);
        return Err(CopyError::Mismatch {
            path: target.to_path_buf(),
        });
    }
    if target.exists() {
        return Err(CopyError::TargetExists(target.to_path_buf()));
    }
    fs::rename(&part, target).map_err(write_err(target))?;
    sync_parent(target);
    Ok(expected)
}

/// Rend le renommage durable (Unix) ; sans objet sous Windows.
fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent()
        && let Ok(dir) = File::open(parent)
    {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copie_identique_et_date_conservee() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.RAF");
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 7 % 256) as u8).collect();
        fs::write(&source, &data).unwrap();
        let target = dir.path().join("bib").join("2026").join("a.RAF");

        let hash = copy_verified(&source, &target).unwrap();
        assert_eq!(hash, hash_file(&source).unwrap());
        assert_eq!(fs::read(&target).unwrap(), data);
        assert!(!part_path(&target).exists());
        let t1 = fs::metadata(&source).unwrap().modified().unwrap();
        let t2 = fs::metadata(&target).unwrap().modified().unwrap();
        assert_eq!(t1, t2);
    }

    #[test]
    fn n_ecrase_jamais_une_destination() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("s.RAF");
        let target = dir.path().join("t.RAF");
        fs::write(&source, b"nouveau").unwrap();
        fs::write(&target, b"existant").unwrap();
        assert!(matches!(
            copy_verified(&source, &target),
            Err(CopyError::TargetExists(_))
        ));
        assert_eq!(fs::read(&target).unwrap(), b"existant");
    }

    #[test]
    fn reprend_apres_un_fichier_temporaire_abandonne() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("s.RAF");
        let target = dir.path().join("t.RAF");
        fs::write(&source, b"contenu complet").unwrap();
        fs::write(part_path(&target), b"cont").unwrap();
        copy_verified(&source, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"contenu complet");
    }
}
