//! Échecs des tâches de fond, remontés à l'interface au lieu de rester dans la console.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const EVENT_WARNING: &str = "background-warning";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WarningKind {
    /// Les marques n'ont pas pu être écrites dans les XMP ; elles restent en attente.
    Xmp,
    /// Les XMP en attente ont fini par être écrits.
    XmpRecovered,
    /// L'aperçu d'un média n'a pas pu être créé.
    Preview,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub kind: WarningKind,
    /// Fichiers concernés, pour le message traduit.
    pub count: usize,
    /// Cause technique, affichée dans l'infobulle.
    pub detail: String,
}

/// Garde la trace dans la console et prévient l'interface.
pub fn warn(app: &AppHandle, kind: WarningKind, count: usize, detail: impl ToString) {
    let detail = detail.to_string();
    if !detail.is_empty() {
        eprintln!("{kind:?} : {detail}");
    }
    let _ = app.emit(
        EVENT_WARNING,
        Warning {
            kind,
            count,
            detail,
        },
    );
}
