//! Base SQLite : ouverture, réglages de connexion et migrations numérotées.
//!
//! Chaque migration de `migrations/` est appliquée une seule fois, dans une
//! transaction ; la version courante est suivie par `PRAGMA user_version`.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("erreur SQLite : {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "la base est en version {found}, plus récente que cette version d'Afterdusk ({supported})"
    )]
    TooNew { found: u32, supported: u32 },
}

/// Migrations dans l'ordre ; l'indice + 1 est le numéro de version.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001_initial.sql"),
    include_str!("migrations/0002_recherche_supprimable.sql"),
    include_str!("migrations/0003_piles_raw_jpeg.sql"),
    include_str!("migrations/0004_retouches.sql"),
    include_str!("migrations/0005_historique_position.sql"),
    include_str!("migrations/0006_instantanes.sql"),
    include_str!("migrations/0007_apercus_rendus.sql"),
    include_str!("migrations/0008_traces_gpx.sql"),
];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// Ouvre (ou crée) la base et applique les migrations manquantes.
pub fn open(path: &Path) -> Result<Connection, DbError> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Connexion à une base déjà ouverte par [`open`] au démarrage : ni passage en
/// WAL ni migrations, qui demandent un verrou exclusif. Plusieurs fenêtres et
/// fils d'aperçus se connectent en même temps sans se gêner.
pub fn connect(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

/// Base en mémoire, pour les tests.
pub fn open_in_memory() -> Result<Connection, DbError> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<(), DbError> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(DbError::TooNew {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// Instant présent en UTC, au format ISO 8601 stocké en base.
pub fn now_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_appliquees_une_seule_fois() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("afterdusk.db");
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN \
                 ('libraries', 'albums', 'media', 'media_exif', 'imports', 'import_items')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 6);
    }

    #[test]
    fn refuse_une_base_plus_recente() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("afterdusk.db");
        drop(open(&path).unwrap());
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(conn);
        assert!(matches!(open(&path), Err(DbError::TooNew { .. })));
    }

    #[test]
    fn connexions_simultanees() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("afterdusk.db");
        drop(open(&path).unwrap());
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let conn = connect(&path).unwrap();
                    conn.query_row("SELECT count(*) FROM libraries", [], |r| r.get::<_, i64>(0))
                        .unwrap()
                })
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap(), 0);
        }
    }

    #[test]
    fn contraintes_actives() {
        let conn = open_in_memory().unwrap();
        // Album sans bibliothèque : refusé par la clé étrangère.
        let err = conn.execute(
            "INSERT INTO albums (library_id, name, folder, created_at) VALUES (42, 'a', 'a', 'x')",
            [],
        );
        assert!(err.is_err());
    }
}
