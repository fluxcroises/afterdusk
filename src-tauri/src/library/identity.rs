//! Identité du photographe (cahier des charges §4.10) : nom, copyright et site,
//! écrits dans les XMP à l'import (`dc:creator`, `dc:rights`) et repris plus
//! tard par le filigrane. Réglage `identity`, en JSON.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::LibraryError;

const KEY: &str = "identity";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Identity {
    pub name: String,
    /// « © 2026 Prénom Nom » par exemple ; vide : aucun copyright écrit.
    pub copyright: String,
    pub website: String,
}

impl Identity {
    fn trimmed(self) -> Self {
        Self {
            name: self.name.trim().to_owned(),
            copyright: self.copyright.trim().to_owned(),
            website: self.website.trim().to_owned(),
        }
    }
}

pub fn get(conn: &Connection) -> Result<Identity, LibraryError> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [KEY], |r| {
            r.get(0)
        })
        .optional()?;
    // Un réglage illisible ne bloque rien : identité vide.
    Ok(value
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default())
}

pub fn set(conn: &Connection, identity: Identity) -> Result<Identity, LibraryError> {
    let identity = identity.trimmed();
    let json = serde_json::to_string(&identity).unwrap_or_else(|_| "{}".to_owned());
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [KEY, json.as_str()],
    )?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identite_enregistree_et_relue() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(get(&conn).unwrap(), Identity::default());
        let saved = set(
            &conn,
            Identity {
                name: "  Lewis Bedar ".into(),
                copyright: "© 2026 Lewis Bedar".into(),
                website: String::new(),
            },
        )
        .unwrap();
        assert_eq!(saved.name, "Lewis Bedar");
        assert_eq!(get(&conn).unwrap(), saved);
    }
}
