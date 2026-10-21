//! Lieux des photos (cahier des charges, 4.7) : trace GPX, photo de téléphone
//! du même moment, puis saisie manuelle et carte (V0.3).

pub mod gpx;
pub mod locate;
pub mod matching;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum GeoError {
    #[error("trace GPX illisible : {0}")]
    Gpx(String),
    #[error("{0} : {1}")]
    File(PathBuf, String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Library(#[from] crate::library::LibraryError),
}
