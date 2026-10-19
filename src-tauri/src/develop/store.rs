//! Réglages en base (`edits`, `edit_history`) ; le XMP suit par l'écriture
//! différée des marques (`media.xmp_pending`), y compris hors ligne.
//!
//! Historique : chaque enregistrement ajoute une étape après la position
//! courante (`edits.history_position`, 0 = l'original). Annuler, rétablir ou
//! cliquer une étape déplace cette position et remet ses réglages ; un
//! nouveau réglage après une annulation remplace les étapes qui suivaient.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use super::settings::{DevelopSettings, SettingsError};
use crate::db::now_utc;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error("étape {0} absente de l'historique")]
    NoSuchStep(i64),
}

/// Une étape de l'historique : le réglage modifié et l'état qui en résulte.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStep {
    pub position: i64,
    pub label: String,
    pub settings: DevelopSettings,
    pub created_at: String,
}

/// Historique d'une version et position courante (0 = l'original).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub steps: Vec<HistoryStep>,
    pub current: i64,
}

/// Version et position courante, créées au besoin.
fn edit_of(tx: &Transaction, media_id: i64, now: &str) -> Result<(i64, i64), StoreError> {
    tx.execute(
        "INSERT INTO edits (media_id, name, settings, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT (media_id, name) DO NOTHING",
        params![
            media_id,
            ORIGINAL,
            DevelopSettings::default().to_json(),
            now
        ],
    )?;
    Ok(tx.query_row(
        "SELECT id, history_position FROM edits WHERE media_id = ?1 AND name = ?2",
        params![media_id, ORIGINAL],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?)
}

/// Réglages courants de la version et XMP en file d'écriture.
fn set_current(
    tx: &Transaction,
    edit_id: i64,
    media_id: i64,
    json: &str,
    position: i64,
    now: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE edits SET settings = ?2, history_position = ?3, updated_at = ?4 WHERE id = ?1",
        params![edit_id, json, position, now],
    )?;
    tx.execute(
        "UPDATE media SET xmp_pending = 1, updated_at = ?2 WHERE id = ?1",
        params![media_id, now],
    )?;
    Ok(())
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
    let (edit_id, current) = edit_of(&tx, media_id, &now)?;
    // Après une annulation, les étapes annulées sont remplacées par celle-ci.
    tx.execute(
        "DELETE FROM edit_history WHERE edit_id = ?1 AND position > ?2",
        params![edit_id, current],
    )?;
    let position = current + 1;
    tx.execute(
        "INSERT INTO edit_history (edit_id, position, settings, label, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![edit_id, position, json, label, now],
    )?;
    set_current(&tx, edit_id, media_id, &json, position, &now)?;
    tx.commit()?;
    Ok(settings)
}

/// Historique de la version originale, du plus ancien au plus récent.
pub fn history_of(conn: &Connection, media_id: i64) -> Result<History, StoreError> {
    let Some((edit_id, current)) = conn
        .query_row(
            "SELECT id, history_position FROM edits WHERE media_id = ?1 AND name = ?2",
            params![media_id, ORIGINAL],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()?
    else {
        return Ok(History {
            steps: Vec::new(),
            current: 0,
        });
    };
    let mut stmt = conn.prepare(
        "SELECT position, label, settings, created_at FROM edit_history
         WHERE edit_id = ?1 ORDER BY position",
    )?;
    let rows = stmt
        .query_map([edit_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get(1)?,
                r.get::<_, String>(2)?,
                r.get(3)?,
            ))
        })?
        .collect::<Result<Vec<(i64, String, String, String)>, _>>()?;
    let steps = rows
        .into_iter()
        .map(|(position, label, json, created_at)| {
            Ok(HistoryStep {
                position,
                label,
                settings: DevelopSettings::from_json(&json)?,
                created_at,
            })
        })
        .collect::<Result<_, StoreError>>()?;
    Ok(History { steps, current })
}

/// Revient à une étape de l'historique (0 = l'original) et renvoie ses réglages.
pub fn go_to(
    conn: &mut Connection,
    media_id: i64,
    position: i64,
) -> Result<DevelopSettings, StoreError> {
    let now = now_utc();
    let tx = conn.transaction()?;
    let (edit_id, _) = edit_of(&tx, media_id, &now)?;
    let settings = if position == 0 {
        DevelopSettings::default()
    } else {
        let json: String = tx
            .query_row(
                "SELECT settings FROM edit_history WHERE edit_id = ?1 AND position = ?2",
                params![edit_id, position],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StoreError::NoSuchStep(position))?;
        DevelopSettings::from_json(&json)?
    };
    set_current(&tx, edit_id, media_id, &settings.to_json(), position, &now)?;
    tx.commit()?;
    Ok(settings)
}
