//! Import automatique d'une carte connue (cahier des charges, 4.1 bis ;
//! décision 0009). Une règle relie le numéro de série d'un boîtier aux
//! réglages d'un import : à l'insertion d'une carte, ses photos sont lues
//! (sans rien écrire), et celles d'un boîtier connu partent à l'import avec
//! ces réglages. Le nom d'album n'est pas gardé : chaque import prend le nom
//! par défaut, sinon toutes les séances iraient dans le même album.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::scan::{self, SourceFile};
use super::{ImportError, ImportOptions, known_fingerprints};
use crate::db::now_utc;

/// Version des réglages enregistrés ; toute évolution prévoit une migration
/// dans `decode`.
pub const SETTINGS_VERSION: u32 = 1;

/// Réglages repris à chaque import automatique.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoSettings {
    pub album_per_day: bool,
    pub rename: bool,
    pub backup: bool,
    pub keywords: Vec<String>,
    pub rights: Option<String>,
    pub skip_pair_jpegs: bool,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    settings: AutoSettings,
}

fn encode(settings: &AutoSettings) -> String {
    serde_json::to_string(&Stored {
        version: SETTINGS_VERSION,
        settings: settings.clone(),
    })
    .unwrap_or_else(|_| "{}".to_owned())
}

/// Réglages enregistrés ; un texte illisible donne les réglages par défaut,
/// pour que la liste des règles reste affichable.
fn decode(json: &str) -> AutoSettings {
    match serde_json::from_str::<Stored>(json) {
        // Version 1 : format actuel.
        Ok(stored) if stored.version <= SETTINGS_VERSION => stored.settings,
        _ => AutoSettings::default(),
    }
}

/// Boîtier d'un import, candidat à une règle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    pub serial: String,
    pub camera: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: i64,
    pub body_serial: String,
    pub camera: String,
    pub library_id: i64,
    pub library_name: String,
    pub settings: AutoSettings,
    pub last_used_at: Option<String>,
}

/// Ce que l'insertion d'une carte a donné.
#[derive(Debug, Clone)]
pub enum CardPlan {
    /// Aucune photo d'un boîtier connu : le comportement habituel s'applique.
    Unknown,
    /// Boîtier connu, mais tout est déjà dans la bibliothèque.
    NothingNew { camera: String },
    /// Import à lancer.
    Import {
        camera: String,
        rule_id: i64,
        options: Box<ImportOptions>,
    },
}

/// Boîtiers à numéro de série d'un import, sans règle pour l'instant.
pub fn candidates(conn: &Connection, import_id: i64) -> Result<Vec<Body>, ImportError> {
    let rows = conn
        .prepare(
            "SELECT DISTINCT a.serial, g.make || ' ' || g.model
               FROM media m
               JOIN gear g ON g.id = m.camera_id
               JOIN gear_aliases a ON a.gear_id = g.id
              WHERE m.import_id = ?1 AND a.serial <> ''
                AND a.serial NOT IN (SELECT body_serial FROM auto_import_rules)
              ORDER BY 2, 1",
        )?
        .query_map([import_id], |r| {
            Ok(Body {
                serial: r.get(0)?,
                camera: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Crée une règle par boîtier de l'import, avec ces réglages et la
/// bibliothèque de l'import ; renvoie le nombre de règles créées.
pub fn add_from_import(
    conn: &Connection,
    import_id: i64,
    settings: &AutoSettings,
) -> Result<usize, ImportError> {
    let library_id: i64 = conn
        .query_row(
            "SELECT library_id FROM imports WHERE id = ?1",
            [import_id],
            |r| r.get(0),
        )
        .map_err(|_| ImportError::NotFound(import_id))?;
    let mut added = 0;
    for body in candidates(conn, import_id)? {
        added += conn.execute(
            "INSERT OR IGNORE INTO auto_import_rules
               (body_serial, camera, library_id, options, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                body.serial,
                body.camera,
                library_id,
                encode(settings),
                now_utc()
            ],
        )?;
    }
    Ok(added)
}

pub fn list(conn: &Connection) -> Result<Vec<Rule>, ImportError> {
    let rows = conn
        .prepare(
            "SELECT r.id, r.body_serial, r.camera, r.library_id, l.name, r.options, r.last_used_at
               FROM auto_import_rules r JOIN libraries l ON l.id = r.library_id
              ORDER BY r.camera, r.body_serial",
        )?
        .query_map([], |r| {
            Ok(Rule {
                id: r.get(0)?,
                body_serial: r.get(1)?,
                camera: r.get(2)?,
                library_id: r.get(3)?,
                library_name: r.get(4)?,
                settings: decode(&r.get::<_, String>(5)?),
                last_used_at: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), ImportError> {
    if conn.execute("DELETE FROM auto_import_rules WHERE id = ?1", [id])? == 0 {
        return Err(ImportError::NotFound(id));
    }
    Ok(())
}

/// Lit la carte (sans rien y écrire) et prépare l'import des photos des
/// boîtiers connus. Une carte de plusieurs boîtiers suit la règle du
/// premier rencontré ; ses autres boîtiers connus de la même bibliothèque
/// sont importés avec elle.
pub fn plan_for_card(
    conn: &Connection,
    root: &Path,
    source_label: &str,
) -> Result<CardPlan, ImportError> {
    let rules: HashMap<String, Rule> = list(conn)?
        .into_iter()
        .map(|r| (r.body_serial.clone(), r))
        .collect();
    if rules.is_empty() {
        return Ok(CardPlan::Unknown);
    }
    let scanned = scan::scan(root)?;
    let rule_of = |f: &SourceFile| {
        f.metadata
            .body_serial
            .as_deref()
            .map(str::trim)
            .and_then(|s| rules.get(s))
    };
    let Some(rule) = scanned.files.iter().find_map(rule_of) else {
        return Ok(CardPlan::Unknown);
    };
    let known = known_fingerprints(conn, rule.library_id)?;
    // Les fichiers sans numéro (JPEG compagnon d'un RAW, parfois) suivent
    // leur déclenchement : on garde tout ce qui n'est pas d'un autre boîtier.
    let files: Vec<_> = scanned
        .files
        .iter()
        .filter(|f| match f.metadata.body_serial.as_deref().map(str::trim) {
            Some(serial) if !serial.is_empty() => rules
                .get(serial)
                .is_some_and(|r| r.library_id == rule.library_id),
            _ => true,
        })
        .filter(|f| !known.contains(&f.fingerprint))
        .map(|f| f.path.clone())
        .collect();
    if files.is_empty() {
        return Ok(CardPlan::NothingNew {
            camera: rule.camera.clone(),
        });
    }
    let s = &rule.settings;
    Ok(CardPlan::Import {
        camera: rule.camera.clone(),
        rule_id: rule.id,
        options: Box::new(ImportOptions {
            library_id: rule.library_id,
            source_path: root.to_path_buf(),
            source_label: source_label.to_owned(),
            files,
            album_name: String::new(),
            album_per_day: s.album_per_day,
            rename: s.rename,
            backup: s.backup,
            keywords: s.keywords.clone(),
            creator: None,
            rights: s.rights.clone(),
            skip_pair_jpegs: s.skip_pair_jpegs,
            client_id: None,
            session_type_id: None,
            session_label: String::new(),
        }),
    })
}

/// Note l'usage d'une règle (affiché dans les réglages).
pub fn touch(conn: &Connection, rule_id: i64) -> Result<(), ImportError> {
    conn.execute(
        "UPDATE auto_import_rules SET last_used_at = ?2 WHERE id = ?1",
        params![rule_id, now_utc()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reglages_versionnes() {
        let s = AutoSettings {
            album_per_day: true,
            rename: true,
            keywords: vec!["mariage".into()],
            ..Default::default()
        };
        assert_eq!(decode(&encode(&s)), s);
        assert_eq!(decode("pas du JSON"), AutoSettings::default());
        assert_eq!(
            decode(r#"{"version":99,"settings":{"rename":true}}"#),
            AutoSettings::default()
        );
    }
}
