//! Réglages en base (`edits`, `edit_history`, `snapshots`) ; le XMP suit par
//! l'écriture différée des marques (`media.xmp_pending`), y compris hors ligne.
//!
//! Versions : l'originale (nom vide) et les copies virtuelles, chacune avec ses
//! réglages, son historique et ses instantanés, sans dupliquer le fichier.
//!
//! Historique : chaque enregistrement ajoute une étape après la position
//! courante (`edits.history_position`, 0 = la photo d'origine, sans réglage).
//! Annuler, rétablir ou cliquer une étape déplace cette position et remet ses
//! réglages ; un nouveau réglage après une annulation remplace les étapes qui
//! suivaient. Les instantanés, eux, restent tant qu'on ne les supprime pas.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};

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
    #[error("version « {0} » introuvable")]
    NoSuchVersion(String),
    #[error("instantané « {0} » introuvable")]
    NoSuchSnapshot(String),
    #[error("le nom « {0} » est déjà pris")]
    NameTaken(String),
    #[error("nom vide ou trop long")]
    InvalidName,
}

/// Version originale d'une photo.
pub const ORIGINAL: &str = "";

/// Longueur maximale d'un nom de version ou d'instantané, en caractères.
const NAME_MAX: usize = 64;

/// Étiquette d'historique de la création d'une copie virtuelle.
pub const VERSION_CREATED: &str = "version.create";

/// Étiquette d'historique de l'application d'un instantané.
pub fn snapshot_label(name: &str) -> String {
    format!("snapshot:{name}")
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

/// Historique d'une version et position courante (0 = la photo d'origine).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub steps: Vec<HistoryStep>,
    pub current: i64,
}

/// Un instantané nommé : un état de réglages gardé de côté.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub name: String,
    pub settings: DevelopSettings,
    pub created_at: String,
}

/// Une copie virtuelle.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub name: String,
    pub created_at: String,
}

/// Ce que le XMP reçoit (JSON), chaque propriété absente si elle est vide.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DevelopXmp {
    /// Réglages de la version originale (`afterdusk:Develop`).
    pub develop: Option<String>,
    /// Instantanés de l'originale (`afterdusk:Snapshots`).
    pub snapshots: Option<String>,
    /// Copies virtuelles avec leurs réglages et instantanés (`afterdusk:Versions`).
    pub versions: Option<String>,
}

fn clean_name(name: &str) -> Result<String, StoreError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(StoreError::InvalidName);
    }
    Ok(name.to_owned())
}

/// Identifiant et position courante d'une version ; l'originale est créée au
/// besoin, une copie doit exister.
fn edit_of(
    tx: &Transaction,
    media_id: i64,
    version: &str,
    now: &str,
) -> Result<(i64, i64), StoreError> {
    if version == ORIGINAL {
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
    }
    tx.query_row(
        "SELECT id, history_position FROM edits WHERE media_id = ?1 AND name = ?2",
        params![media_id, version],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()?
    .ok_or_else(|| StoreError::NoSuchVersion(version.to_owned()))
}

fn mark_xmp(tx: &Transaction, media_id: i64, now: &str) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE media SET xmp_pending = xmp_pending + 1, updated_at = ?2 WHERE id = ?1",
        params![media_id, now],
    )?;
    Ok(())
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
    mark_xmp(tx, media_id, now)
}

/// Ajoute une étape après la position courante (les étapes annulées sont
/// remplacées) et en fait l'état courant.
fn push_step(
    tx: &Transaction,
    edit_id: i64,
    media_id: i64,
    current: i64,
    json: &str,
    label: &str,
    now: &str,
) -> Result<(), StoreError> {
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
    set_current(tx, edit_id, media_id, json, position, now)
}

fn is_unique_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

/// Réglages d'une version ; rendu neutre pour une originale jamais retouchée.
pub fn load(
    conn: &Connection,
    media_id: i64,
    version: &str,
) -> Result<DevelopSettings, StoreError> {
    let json: Option<String> = conn
        .query_row(
            "SELECT settings FROM edits WHERE media_id = ?1 AND name = ?2",
            params![media_id, version],
            |r| r.get(0),
        )
        .optional()?;
    match json {
        Some(json) => Ok(DevelopSettings::from_json(&json)?),
        None if version == ORIGINAL => Ok(DevelopSettings::default()),
        None => Err(StoreError::NoSuchVersion(version.to_owned())),
    }
}

/// Enregistre les réglages d'une version, ajoute une étape à l'historique et
/// met le XMP en file d'écriture. Renvoie les réglages tels qu'enregistrés.
pub fn save(
    conn: &mut Connection,
    media_id: i64,
    version: &str,
    settings: DevelopSettings,
    label: &str,
) -> Result<DevelopSettings, StoreError> {
    let settings = settings.sanitized();
    let json = settings.to_json();
    let now = now_utc();
    let tx = conn.transaction()?;
    let (edit_id, current) = edit_of(&tx, media_id, version, &now)?;
    push_step(&tx, edit_id, media_id, current, &json, label, &now)?;
    tx.commit()?;
    Ok(settings)
}

/// Historique d'une version, du plus ancien au plus récent.
pub fn history_of(conn: &Connection, media_id: i64, version: &str) -> Result<History, StoreError> {
    let Some((edit_id, current)) = conn
        .query_row(
            "SELECT id, history_position FROM edits WHERE media_id = ?1 AND name = ?2",
            params![media_id, version],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()?
    else {
        if version != ORIGINAL {
            return Err(StoreError::NoSuchVersion(version.to_owned()));
        }
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

/// Revient à une étape de l'historique (0 = la photo d'origine) et renvoie
/// ses réglages.
pub fn go_to(
    conn: &mut Connection,
    media_id: i64,
    version: &str,
    position: i64,
) -> Result<DevelopSettings, StoreError> {
    let now = now_utc();
    let tx = conn.transaction()?;
    let (edit_id, _) = edit_of(&tx, media_id, version, &now)?;
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

/// Copies virtuelles d'une photo, de la plus ancienne à la plus récente.
pub fn versions(conn: &Connection, media_id: i64) -> Result<Vec<Version>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT name, created_at FROM edits WHERE media_id = ?1 AND name <> ?2 ORDER BY id",
    )?;
    let list = stmt
        .query_map(params![media_id, ORIGINAL], |r| {
            Ok(Version {
                name: r.get(0)?,
                created_at: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(list)
}

/// Crée une copie virtuelle à partir des réglages de la version `from` ; son
/// historique commence par cette création.
pub fn create_version(
    conn: &mut Connection,
    media_id: i64,
    from: &str,
    name: &str,
) -> Result<DevelopSettings, StoreError> {
    let name = clean_name(name)?;
    let settings = load(conn, media_id, from)?;
    let json = settings.to_json();
    let now = now_utc();
    let tx = conn.transaction()?;
    let inserted = tx.execute(
        "INSERT INTO edits (media_id, name, settings, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![media_id, name, json, now],
    );
    match inserted {
        Err(e) if is_unique_violation(&e) => return Err(StoreError::NameTaken(name)),
        other => other?,
    };
    let edit_id = tx.last_insert_rowid();
    push_step(&tx, edit_id, media_id, 0, &json, VERSION_CREATED, &now)?;
    tx.commit()?;
    Ok(settings)
}

pub fn rename_version(
    conn: &mut Connection,
    media_id: i64,
    from: &str,
    to: &str,
) -> Result<(), StoreError> {
    if from == ORIGINAL {
        return Err(StoreError::NoSuchVersion(from.to_owned()));
    }
    let to = clean_name(to)?;
    let now = now_utc();
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE edits SET name = ?3, updated_at = ?4 WHERE media_id = ?1 AND name = ?2",
        params![media_id, from, to, now],
    );
    match changed {
        Err(e) if is_unique_violation(&e) => return Err(StoreError::NameTaken(to)),
        Ok(0) => return Err(StoreError::NoSuchVersion(from.to_owned())),
        other => other?,
    };
    mark_xmp(&tx, media_id, &now)?;
    tx.commit()?;
    Ok(())
}

/// Supprime une copie virtuelle (jamais l'originale), avec son historique et
/// ses instantanés. Aucun fichier n'est touché.
pub fn delete_version(conn: &mut Connection, media_id: i64, name: &str) -> Result<(), StoreError> {
    if name == ORIGINAL {
        return Err(StoreError::NoSuchVersion(name.to_owned()));
    }
    let now = now_utc();
    let tx = conn.transaction()?;
    let deleted = tx.execute(
        "DELETE FROM edits WHERE media_id = ?1 AND name = ?2",
        params![media_id, name],
    )?;
    if deleted == 0 {
        return Err(StoreError::NoSuchVersion(name.to_owned()));
    }
    mark_xmp(&tx, media_id, &now)?;
    tx.commit()?;
    Ok(())
}

/// Instantanés d'une version, du plus ancien au plus récent.
pub fn snapshots(
    conn: &Connection,
    media_id: i64,
    version: &str,
) -> Result<Vec<Snapshot>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT s.name, s.settings, s.created_at FROM snapshots s
         JOIN edits e ON e.id = s.edit_id
         WHERE e.media_id = ?1 AND e.name = ?2 ORDER BY s.id",
    )?;
    let rows = stmt
        .query_map(params![media_id, version], |r| {
            Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?))
        })?
        .collect::<Result<Vec<(String, String, String)>, _>>()?;
    rows.into_iter()
        .map(|(name, json, created_at)| {
            Ok(Snapshot {
                name,
                settings: DevelopSettings::from_json(&json)?,
                created_at,
            })
        })
        .collect()
}

/// Garde les réglages courants d'une version sous un nom.
pub fn create_snapshot(
    conn: &mut Connection,
    media_id: i64,
    version: &str,
    name: &str,
) -> Result<(), StoreError> {
    let name = clean_name(name)?;
    let settings = load(conn, media_id, version)?;
    let now = now_utc();
    let tx = conn.transaction()?;
    let (edit_id, _) = edit_of(&tx, media_id, version, &now)?;
    let inserted = tx.execute(
        "INSERT INTO snapshots (edit_id, name, settings, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![edit_id, name, settings.to_json(), now],
    );
    match inserted {
        Err(e) if is_unique_violation(&e) => return Err(StoreError::NameTaken(name)),
        other => other?,
    };
    mark_xmp(&tx, media_id, &now)?;
    tx.commit()?;
    Ok(())
}

/// Applique un instantané : une étape d'historique comme une autre, donc annulable.
pub fn apply_snapshot(
    conn: &mut Connection,
    media_id: i64,
    version: &str,
    name: &str,
) -> Result<DevelopSettings, StoreError> {
    let json: String = conn
        .query_row(
            "SELECT s.settings FROM snapshots s JOIN edits e ON e.id = s.edit_id
             WHERE e.media_id = ?1 AND e.name = ?2 AND s.name = ?3",
            params![media_id, version, name],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| StoreError::NoSuchSnapshot(name.to_owned()))?;
    let settings = DevelopSettings::from_json(&json)?;
    save(conn, media_id, version, settings, &snapshot_label(name))
}

pub fn rename_snapshot(
    conn: &mut Connection,
    media_id: i64,
    version: &str,
    from: &str,
    to: &str,
) -> Result<(), StoreError> {
    let to = clean_name(to)?;
    let now = now_utc();
    let tx = conn.transaction()?;
    let (edit_id, _) = edit_of(&tx, media_id, version, &now)?;
    let changed = tx.execute(
        "UPDATE snapshots SET name = ?3 WHERE edit_id = ?1 AND name = ?2",
        params![edit_id, from, to],
    );
    match changed {
        Err(e) if is_unique_violation(&e) => return Err(StoreError::NameTaken(to)),
        Ok(0) => return Err(StoreError::NoSuchSnapshot(from.to_owned())),
        other => other?,
    };
    mark_xmp(&tx, media_id, &now)?;
    tx.commit()?;
    Ok(())
}

pub fn delete_snapshot(
    conn: &mut Connection,
    media_id: i64,
    version: &str,
    name: &str,
) -> Result<(), StoreError> {
    let now = now_utc();
    let tx = conn.transaction()?;
    let (edit_id, _) = edit_of(&tx, media_id, version, &now)?;
    let deleted = tx.execute(
        "DELETE FROM snapshots WHERE edit_id = ?1 AND name = ?2",
        params![edit_id, name],
    )?;
    if deleted == 0 {
        return Err(StoreError::NoSuchSnapshot(name.to_owned()));
    }
    mark_xmp(&tx, media_id, &now)?;
    tx.commit()?;
    Ok(())
}

fn settings_value(settings: &DevelopSettings) -> Value {
    serde_json::to_value(settings).unwrap_or(Value::Null)
}

fn snapshots_value(list: &[Snapshot]) -> Value {
    Value::Array(
        list.iter()
            .map(|s| json!({ "name": s.name, "settings": settings_value(&s.settings) }))
            .collect(),
    )
}

/// Une étape d'historique, et les instantanés, d'une version reprise du XMP.
fn restore_version(
    tx: &Transaction,
    media_id: i64,
    name: &str,
    settings: &DevelopSettings,
    snapshots: Option<&Value>,
    now: &str,
) -> Result<(), StoreError> {
    let json = settings.to_json();
    tx.execute(
        "INSERT INTO edits (media_id, name, settings, history_position, created_at, updated_at)
         VALUES (?1, ?2, ?3, 1, ?4, ?4)",
        params![media_id, name, json, now],
    )?;
    let edit_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO edit_history (edit_id, position, settings, label, created_at)
         VALUES (?1, 1, ?2, 'xmp.restore', ?3)",
        params![edit_id, json, now],
    )?;
    for snap in snapshots.and_then(Value::as_array).into_iter().flatten() {
        let (Some(snap_name), Some(snap_settings)) = (
            snap.get("name").and_then(Value::as_str),
            snap.get("settings"),
        ) else {
            continue;
        };
        let Ok(snap_settings) = DevelopSettings::from_json(&snap_settings.to_string()) else {
            continue;
        };
        let Ok(snap_name) = clean_name(snap_name) else {
            continue;
        };
        tx.execute(
            "INSERT OR IGNORE INTO snapshots (edit_id, name, settings, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![edit_id, snap_name, snap_settings.to_json(), now],
        )?;
    }
    Ok(())
}

/// Retouches d'une photo reprises de son XMP (`afterdusk:Develop`,
/// `afterdusk:Snapshots`, `afterdusk:Versions`), après reconstruction de la
/// base. Le XMP n'est pas réécrit. Une photo qui a déjà des retouches en base
/// n'est pas touchée ; une partie illisible est ignorée. Renvoie vrai si
/// quelque chose a été repris.
pub fn restore_from_xmp(
    conn: &mut Connection,
    media_id: i64,
    develop: Option<&str>,
    snapshots: Option<&str>,
    versions: Option<&str>,
) -> Result<bool, StoreError> {
    if develop.is_none() && snapshots.is_none() && versions.is_none() {
        return Ok(false);
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM edits WHERE media_id = ?1)",
        [media_id],
        |r| r.get(0),
    )?;
    if exists {
        return Ok(false);
    }
    let parse = |text: Option<&str>| text.and_then(|t| serde_json::from_str::<Value>(t).ok());
    let settings = develop
        .and_then(|d| DevelopSettings::from_json(d).ok())
        .unwrap_or_default();
    let snaps = parse(snapshots);
    let copies = parse(versions);
    let now = now_utc();
    let tx = conn.transaction()?;
    restore_version(&tx, media_id, ORIGINAL, &settings, snaps.as_ref(), &now)?;
    for copy in copies
        .as_ref()
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = copy.get("name").and_then(Value::as_str) else {
            continue;
        };
        let (Ok(name), Some(Ok(settings))) = (
            clean_name(name),
            copy.get("settings")
                .map(|v| DevelopSettings::from_json(&v.to_string())),
        ) else {
            continue;
        };
        if name == ORIGINAL {
            continue;
        }
        restore_version(&tx, media_id, &name, &settings, copy.get("snapshots"), &now)?;
    }
    tx.commit()?;
    Ok(true)
}

/// Ce que le XMP de la photo doit contenir : réglages de l'originale,
/// instantanés et copies virtuelles. Rien pour une photo jamais retouchée.
pub fn xmp_data(conn: &Connection, media_id: i64) -> Result<DevelopXmp, StoreError> {
    let settings = load(conn, media_id, ORIGINAL)?;
    let snaps = snapshots(conn, media_id, ORIGINAL)?;
    let copies = versions(conn, media_id)?
        .into_iter()
        .map(|v| {
            let settings = load(conn, media_id, &v.name)?;
            let snaps = snapshots(conn, media_id, &v.name)?;
            Ok(json!({
                "name": v.name,
                "settings": settings_value(&settings),
                "snapshots": snapshots_value(&snaps),
            }))
        })
        .collect::<Result<Vec<Value>, StoreError>>()?;
    Ok(DevelopXmp {
        develop: (!settings.is_neutral()).then(|| settings.to_json()),
        snapshots: (!snaps.is_empty()).then(|| snapshots_value(&snaps).to_string()),
        versions: (!copies.is_empty()).then(|| Value::Array(copies).to_string()),
    })
}
