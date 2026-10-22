//! Profil d'interface (cahier des charges §6, V0.5) : Simplifiée, De base ou
//! Complète. Le profil ne fait que masquer des fonctions dans l'interface ;
//! ce qui est masqué continue de tourner et les données restent intactes.
//! Sans profil enregistré (installation antérieure à la V0.5), l'interface
//! reste complète.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

const KEY: &str = "interface.profile";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Profile {
    Simple,
    Basic,
    #[default]
    Complete,
}

/// Profil enregistré ; complet s'il n'y en a pas ou s'il est illisible.
pub fn get(conn: &Connection) -> rusqlite::Result<Profile> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [KEY], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(value
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default())
}

pub fn set(conn: &Connection, profile: Profile) -> rusqlite::Result<()> {
    let json = serde_json::to_string(&profile).unwrap_or_else(|_| "\"complete\"".into());
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = ?2",
        params![KEY, json],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complet_par_defaut_puis_enregistre() {
        let conn = crate::db::open_in_memory().unwrap();
        // Installation existante : rien d'enregistré, interface complète.
        assert_eq!(get(&conn).unwrap(), Profile::Complete);
        set(&conn, Profile::Simple).unwrap();
        assert_eq!(get(&conn).unwrap(), Profile::Simple);
        set(&conn, Profile::Basic).unwrap();
        assert_eq!(get(&conn).unwrap(), Profile::Basic);
        conn.execute(
            "UPDATE settings SET value = '\"futur\"' WHERE key = ?1",
            [KEY],
        )
        .unwrap();
        assert_eq!(get(&conn).unwrap(), Profile::Complete);
    }
}
