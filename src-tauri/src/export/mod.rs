//! Export (cahier des charges, 4.6) : rendu des photos retouchées, formats,
//! espaces colorimétriques, et copie vérifiée des originaux.

pub mod color;
pub mod encode;
pub mod render;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("la destination {0} existe déjà")]
    TargetExists(PathBuf),
    #[error("image trop grande pour ce format")]
    TooLarge,
    #[error("conversion de couleurs impossible : {0}")]
    Color(String),
    #[error("encodage impossible : {0}")]
    Encode(String),
    #[error(transparent)]
    Raw(#[from] crate::raw::RawError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
