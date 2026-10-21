//! Export (cahier des charges, 4.6) : rendu des photos retouchées, formats,
//! espaces colorimétriques, et copie vérifiée des originaux.

pub mod color;
pub mod contact_sheet;
pub mod encode;
pub mod heif;
pub mod heif_meta;
pub mod job;
pub mod metadata;
pub mod presets;
pub mod render;
pub mod watermark;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("la destination {0} existe déjà")]
    TargetExists(PathBuf),
    #[error("image trop grande pour ce format")]
    TooLarge,
    #[error("conversion de couleurs impossible : {0}")]
    Color(String),
    #[error("métadonnées impossibles à écrire : {0}")]
    Metadata(String),
    #[error("filigrane impossible : {0}")]
    Watermark(String),
    #[error("encodage impossible : {0}")]
    Encode(String),
    #[error("aucun encodeur HEIF sur ce système")]
    HeifUnavailable,
    #[error("réglages d'export illisibles : {0}")]
    Settings(String),
    #[error("copie de l'original impossible : {0}")]
    Copy(String),
    #[error("export rendu réservé aux RAW pour l'instant : {0}")]
    Unsupported(String),
    #[error(transparent)]
    Raw(#[from] crate::raw::RawError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
