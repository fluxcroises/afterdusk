//! Analyse d'une source (carte, boîtier, dossier) : liste des médias,
//! métadonnées et empreintes. Lecture seule : rien n'est écrit sur la source.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use walkdir::WalkDir;

use crate::metadata::media::{MediaFormat, MediaMetadata, format_of, read_metadata};

/// Octets lus au début et à la fin du fichier pour l'empreinte.
const PARTIAL_HASH_BYTES: u64 = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("la source {0} n'existe pas ou n'est pas un dossier")]
    NotADirectory(PathBuf),
    #[error("lecture impossible de {path} : {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    pub path: PathBuf,
    pub file_name: String,
    pub size: u64,
    /// Date de modification sur la source, ISO 8601 UTC.
    pub modified: Option<String>,
    pub format: MediaFormat,
    /// Date de prise de vue, ou à défaut date de modification en heure locale.
    pub captured_at: String,
    /// La date vient-elle des métadonnées ?
    pub has_capture_date: bool,
    pub fingerprint: String,
    #[serde(skip)]
    pub metadata: MediaMetadata,
}

impl SourceFile {
    /// Jour de prise de vue `AAAA-MM-JJ`, pour le regroupement.
    pub fn day(&self) -> &str {
        &self.captured_at[..10.min(self.captured_at.len())]
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub files: Vec<SourceFile>,
    /// Fichiers visibles non importés (vidéos en V0.1, formats inconnus).
    pub ignored: usize,
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.') || name.eq_ignore_ascii_case("System Volume Information")
}

/// Parcourt la source et lit chaque média reconnu.
pub fn scan(root: &Path) -> Result<ScanResult, ScanError> {
    if !root.is_dir() {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }
    let mut result = ScanResult::default();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_hidden(&e.file_name().to_string_lossy()));
    for entry in walker {
        let Ok(entry) = entry else {
            result.ignored += 1;
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(format) = format_of(path) else {
            result.ignored += 1;
            continue;
        };
        result.files.push(describe(path, format)?);
    }
    result
        .files
        .sort_by(|a, b| (&a.captured_at, &a.file_name).cmp(&(&b.captured_at, &b.file_name)));
    Ok(result)
}

/// Décrit un fichier de la source.
pub fn describe(path: &Path, format: MediaFormat) -> Result<SourceFile, ScanError> {
    let io = |source| ScanError::Io {
        path: path.to_path_buf(),
        source,
    };
    let meta = std::fs::metadata(path).map_err(io)?;
    let modified = meta.modified().ok();
    let metadata = read_metadata(path, format);
    let (captured_at, has_capture_date) = match &metadata.captured_at {
        Some(date) => (date.clone(), true),
        None => (modified.map(local_datetime).unwrap_or_default(), false),
    };
    let fingerprint = fingerprint(path, meta.len(), &captured_at).map_err(io)?;
    Ok(SourceFile {
        path: path.to_path_buf(),
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size: meta.len(),
        modified: modified.map(utc_datetime),
        format,
        captured_at,
        has_capture_date,
        fingerprint,
        metadata,
    })
}

/// Empreinte de détection des doublons : `taille|prise de vue|BLAKE3(début + fin)`.
/// Le nom n'en fait pas partie, pour reconnaître aussi une photo renommée.
pub fn fingerprint(path: &Path, size: u64, captured_at: &str) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; PARTIAL_HASH_BYTES as usize];
    let head = read_up_to(&mut file, &mut buf)?;
    hasher.update(&buf[..head]);
    if size > 2 * PARTIAL_HASH_BYTES {
        file.seek(SeekFrom::End(-(PARTIAL_HASH_BYTES as i64)))?;
        let tail = read_up_to(&mut file, &mut buf)?;
        hasher.update(&buf[..tail]);
    } else if size > PARTIAL_HASH_BYTES {
        let rest = read_up_to(&mut file, &mut buf)?;
        hasher.update(&buf[..rest]);
    }
    Ok(format!(
        "{size}|{captured_at}|{}",
        hasher.finalize().to_hex()
    ))
}

fn read_up_to(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

fn local_datetime(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(time)
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

fn utc_datetime(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(time)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_les_fichiers_systeme_et_les_videos() {
        let dir = tempfile::tempdir().unwrap();
        let dcim = dir.path().join("DCIM").join("100_FUJI");
        std::fs::create_dir_all(&dcim).unwrap();
        std::fs::create_dir_all(dir.path().join(".Trashes")).unwrap();
        std::fs::write(dir.path().join(".Trashes").join("x.RAF"), b"x").unwrap();
        std::fs::write(dcim.join("DSCF0001.RAF"), b"raf").unwrap();
        std::fs::write(dcim.join("DSCF0001.JPG"), b"jpg").unwrap();
        std::fs::write(dcim.join("DSCF0002.MOV"), b"mov").unwrap();
        std::fs::write(dcim.join(".DS_Store"), b"").unwrap();

        let result = scan(dir.path()).unwrap();
        let names: Vec<_> = result.files.iter().map(|f| f.file_name.as_str()).collect();
        assert_eq!(names, ["DSCF0001.JPG", "DSCF0001.RAF"]);
        // Seule la vidéo compte : les fichiers cachés ne sont pas des médias.
        assert_eq!(result.ignored, 1);
        // Sans EXIF, la date de modification sert de date de prise de vue.
        assert!(!result.files[0].has_capture_date);
        assert_eq!(result.files[0].captured_at.len(), 19);
    }

    #[test]
    fn empreinte_independante_du_nom() {
        let dir = tempfile::tempdir().unwrap();
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let a = dir.path().join("a.RAF");
        let b = dir.path().join("b.RAF");
        std::fs::write(&a, &data).unwrap();
        std::fs::write(&b, &data).unwrap();
        let date = "2026-10-18T21:04:59";
        assert_eq!(
            fingerprint(&a, 300_000, date).unwrap(),
            fingerprint(&b, 300_000, date).unwrap()
        );
        let mut other = data.clone();
        *other.last_mut().unwrap() ^= 1;
        std::fs::write(&b, &other).unwrap();
        assert_ne!(
            fingerprint(&a, 300_000, date).unwrap(),
            fingerprint(&b, 300_000, date).unwrap()
        );
    }
}
