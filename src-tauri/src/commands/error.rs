//! Erreur renvoyée à l'interface : un code stable, traduit par i18next
//! (`errors.<code>`), et un détail technique pour le journal.

use serde::Serialize;

use crate::db::DbError;
use crate::import::ImportError;
use crate::library::LibraryError;
use crate::library::trash::TrashError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub detail: String,
}

pub type CommandResult<T> = Result<T, CommandError>;

impl CommandError {
    pub fn new(code: &'static str, detail: impl ToString) -> Self {
        Self {
            code,
            detail: detail.to_string(),
        }
    }
}

impl From<ImportError> for CommandError {
    fn from(e: ImportError) -> Self {
        let code = match &e {
            ImportError::LibraryOffline(_) => "libraryOffline",
            ImportError::SourceUnavailable(_) => "sourceUnavailable",
            ImportError::UnsupportedFile(_) => "unsupportedFile",
            ImportError::Scan(_) => "sourceUnreadable",
            ImportError::Template(_) => "invalidTemplate",
            ImportError::Library(LibraryError::MissingRoot(_)) => "missingFolder",
            _ => "internal",
        };
        Self::new(code, e)
    }
}

fn library_code(e: &LibraryError) -> &'static str {
    match e {
        LibraryError::MissingRoot(_) => "missingFolder",
        LibraryError::Template(_) => "invalidTemplate",
        LibraryError::NotFound(_) => "libraryNotFound",
        LibraryError::NotInAlbum { .. } => "notInAlbum",
        LibraryError::CollectionNotFound(_) => "collectionNotFound",
        LibraryError::NotSmart(_) | LibraryError::NotManual(_) => "collectionKind",
        LibraryError::EmptyName => "emptyName",
        LibraryError::Logo(_) => "logo",
        LibraryError::ClientNotFound(_) => "clientNotFound",
        LibraryError::SessionTypeNotFound(_) => "sessionTypeNotFound",
        LibraryError::Sqlite(_) => "internal",
    }
}

impl From<LibraryError> for CommandError {
    fn from(e: LibraryError) -> Self {
        Self::new(library_code(&e), e)
    }
}

impl From<TrashError> for CommandError {
    fn from(e: TrashError) -> Self {
        let code = match &e {
            TrashError::Library(inner) => library_code(inner),
            TrashError::Offline(_) => "libraryOffline",
            TrashError::NotFound(_) => "mediaNotFound",
            TrashError::Io { .. } => "fileMoveFailed",
            TrashError::Sqlite(_) | TrashError::Path(_) | TrashError::OutsideTrash(_) => "internal",
        };
        Self::new(code, e)
    }
}

impl From<DbError> for CommandError {
    fn from(e: DbError) -> Self {
        let code = match &e {
            DbError::TooNew { .. } => "databaseTooNew",
            DbError::Sqlite(_) | DbError::ForeignKeys(..) => "internal",
        };
        Self::new(code, e)
    }
}

impl From<crate::import::scan::ScanError> for CommandError {
    fn from(e: crate::import::scan::ScanError) -> Self {
        Self::new("sourceUnreadable", e)
    }
}

impl From<tauri::Error> for CommandError {
    fn from(e: tauri::Error) -> Self {
        Self::new("internal", e)
    }
}
