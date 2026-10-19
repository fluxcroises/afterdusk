//! Réglages en base (`edits`, `edit_history`) ; le XMP suit par l'écriture
//! différée des marques (`media.xmp_pending`), y compris hors ligne.

use rusqlite::{Connection, OptionalExtension, params};

use super::settings::{DevelopSettings, SettingsError};
use crate::db::now_utc;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Settings(#[from] SettingsError),
}

/// Version originale d'une photo (nom vide) ; les copies virtuelles viendront.
const ORIGINAL: &str = "";

/// Réglages de la version originale ; rendu neutre si la photo n'a jamais été retouchée.
pub fn load(conn: &Connection, media_id: i64) -> Result<DevelopSettings, StoreError> {
    let json: Option<String> = conn
        .query_row(
            "SELECT settings FROM edits WHERE media_id = ?1 AND name = ?2",
            params![media_id, ORIGINAL],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match json {
        Some(json) => DevelopSettings::from_json(&json)?,
        None => DevelopSettings::default(),
    })
}

/// JSON des réglages à écrire dans le XMP, ou rien pour une photo neutre.
pub fn xmp_json(conn: &Connection, media_id: i64) -> Result<Option<String>, StoreError> {
    let settings = load(conn, media_id)?;
    Ok((!settings.is_neutral()).then(|| settings.to_json()))
}

/// Enregistre les réglages, ajoute une étape à l'historique et met le XMP
/// en file d'écriture. Renvoie les réglages tels qu'enregistrés (bornés).
pub fn save(
    conn: &mut Connection,
    media_id: i64,
    settings: DevelopSettings,
    label: &str,
) -> Result<DevelopSettings, StoreError> {
    let settings = settings.sanitized();
    let json = settings.to_json();
    let now = now_utc();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO edits (media_id, name, settings, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT (media_id, name) DO UPDATE SET settings = excluded.settings,
                                                    updated_at = excluded.updated_at",
        params![media_id, ORIGINAL, json, now],
    )?;
    let edit_id: i64 = tx.query_row(
        "SELECT id FROM edits WHERE media_id = ?1 AND name = ?2",
        params![media_id, ORIGINAL],
        |r| r.get(0),
    )?;
    tx.execute(
        "INSERT INTO edit_history (edit_id, position, settings, label, created_at)
         VALUES (?1, (SELECT coalesce(max(position), 0) + 1 FROM edit_history WHERE edit_id = ?1),
                 ?2, ?3, ?4)",
        params![edit_id, json, label, now],
    )?;
    tx.execute(
        "UPDATE media SET xmp_pending = 1, updated_at = ?2 WHERE id = ?1",
        params![media_id, now],
    )?;
    tx.commit()?;
    Ok(settings)
}

/// Libellés de l'historique, du plus ancien au plus récent.
pub fn history(conn: &Connection, media_id: i64) -> Result<Vec<String>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT h.label FROM edit_history h JOIN edits e ON e.id = h.edit_id
         WHERE e.media_id = ?1 AND e.name = ?2 ORDER BY h.position",
    )?;
    let labels = stmt
        .query_map(params![media_id, ORIGINAL], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(labels)
}
