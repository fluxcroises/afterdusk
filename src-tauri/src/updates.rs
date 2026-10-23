//! Vérification des mises à jour (décision 0013) : activée par défaut,
//! désactivable dans les Réglages, au plus une fois par jour. La requête ne
//! porte que sur `latest.json` des versions GitHub ; rien ne s'installe sans
//! un clic de l'utilisateur.

use rusqlite::{Connection, OptionalExtension, params};

const KEY_ENABLED: &str = "updates.check";
const KEY_LAST_CHECK: &str = "updates.lastCheck";

/// Délai minimal entre deux vérifications automatiques.
pub const INTERVAL_SECS: i64 = 24 * 60 * 60;

fn read(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .optional()
}

fn write(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = ?2",
        params![key, value],
    )?;
    Ok(())
}

/// Vérification activée ; vrai si rien n'est enregistré ou si c'est illisible.
pub fn enabled(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(read(conn, KEY_ENABLED)?
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or(true))
}

pub fn set_enabled(conn: &Connection, on: bool) -> rusqlite::Result<()> {
    write(conn, KEY_ENABLED, if on { "true" } else { "false" })
}

/// Date de la dernière vérification, en secondes Unix.
pub fn last_check(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    Ok(read(conn, KEY_LAST_CHECK)?.and_then(|v| v.parse().ok()))
}

pub fn set_last_check(conn: &Connection, at: i64) -> rusqlite::Result<()> {
    write(conn, KEY_LAST_CHECK, &at.to_string())
}

/// Une vérification est due si elle est activée et si la dernière date d'au
/// moins un jour (ou d'une date future : horloge corrigée entre-temps).
pub fn due(enabled: bool, last: Option<i64>, now: i64) -> bool {
    enabled && last.is_none_or(|at| now < at || now - at >= INTERVAL_SECS)
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activee_par_defaut_puis_desactivee() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(enabled(&conn).unwrap());
        set_enabled(&conn, false).unwrap();
        assert!(!enabled(&conn).unwrap());
        set_enabled(&conn, true).unwrap();
        assert!(enabled(&conn).unwrap());
    }

    #[test]
    fn au_plus_une_fois_par_jour() {
        let now = 1_800_000_000;
        assert!(due(true, None, now));
        assert!(!due(true, Some(now - 3600), now));
        assert!(due(true, Some(now - INTERVAL_SECS), now));
        // Horloge remise à l'heure après une date future.
        assert!(due(true, Some(now + 3600), now));
        // Désactivée : jamais, même la première fois.
        assert!(!due(false, None, now));
    }

    #[test]
    fn date_de_la_derniere_verification() {
        let conn = crate::db::open_in_memory().unwrap();
        assert_eq!(last_check(&conn).unwrap(), None);
        set_last_check(&conn, 1_800_000_000).unwrap();
        assert_eq!(last_check(&conn).unwrap(), Some(1_800_000_000));
    }
}
