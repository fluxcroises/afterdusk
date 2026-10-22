//! Export des extraits d'un clip (dérushage, cahier des charges 4.9) :
//! copie des flux sans réencodage, un fichier par extrait ou tous réunis en
//! un seul. L'original n'est qu'ouvert en lecture ; rien n'est écrit à côté.

use std::path::{Path, PathBuf};

use super::ExportError;
use super::presets;
use crate::library::clips::{Clip, stamp};
use crate::video;

/// Fichiers intermédiaires de la réunion, effacés en fin d'export.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, ExportError> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir =
            std::env::temp_dir().join(format!("afterdusk-extraits-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Exporte les extraits de `source` dans `dir` : un fichier par extrait,
/// ou un seul fichier qui les réunit dans l'ordre (`join`). `progress`
/// reçoit (fait, total). Renvoie les fichiers écrits.
pub fn export(
    source: &Path,
    clips: &[Clip],
    dir: &Path,
    join: bool,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<PathBuf>, ExportError> {
    if clips.is_empty() {
        return Ok(Vec::new());
    }
    if !video::available() {
        return Err(ExportError::Unsupported("FFmpeg absent".into()));
    }
    std::fs::create_dir_all(dir)?;
    let stem = source
        .file_stem()
        .map_or_else(|| "Vidéo".into(), |s| s.to_string_lossy().into_owned());
    let ext = source
        .extension()
        .map_or_else(|| "mp4".into(), |e| e.to_string_lossy().to_lowercase());
    // Images clés relues : l'extrait part de celle qui précède son début.
    let keys = video::keyframes(source).unwrap_or_default();
    let total = clips.len() + usize::from(join);
    let failed = |c: &Clip| {
        ExportError::Encode(format!(
            "extrait {}-{} de {}",
            stamp(c.start_ms),
            stamp(c.end_ms),
            source.display()
        ))
    };
    if join {
        let scratch = Scratch::new()?;
        let mut parts = Vec::with_capacity(clips.len());
        for (i, c) in clips.iter().enumerate() {
            let part = scratch.0.join(format!("{i:03}.{ext}"));
            if !video::cut(source, video::snap(&keys, c.start_ms), c.end_ms, &part) {
                return Err(failed(c));
            }
            parts.push(part);
            progress(i + 1, total);
        }
        let out = presets::free_path(dir, &format!("{stem} - extraits"), &ext);
        if !video::concat(&parts, &out) {
            return Err(ExportError::Encode(format!(
                "réunion des extraits de {}",
                source.display()
            )));
        }
        progress(total, total);
        return Ok(vec![out]);
    }
    let mut written = Vec::with_capacity(clips.len());
    for (i, c) in clips.iter().enumerate() {
        let start = video::snap(&keys, c.start_ms);
        let label = if c.name.trim().is_empty() {
            format!("{}-{}", stamp(start), stamp(c.end_ms))
        } else {
            presets::clean_name(&c.name)
        };
        let out = presets::free_path(dir, &format!("{stem} - {label}"), &ext);
        if !video::cut(source, start, c.end_ms, &out) {
            return Err(failed(c));
        }
        written.push(out);
        progress(i + 1, total);
    }
    Ok(written)
}
