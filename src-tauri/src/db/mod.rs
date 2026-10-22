//! Base SQLite : ouverture, réglages de connexion et migrations numérotées.
//!
//! Chaque migration de `migrations/` est appliquée une seule fois, dans une
//! transaction ; la version courante est suivie par `PRAGMA user_version`.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("erreur SQLite : {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "la base est en version {found}, plus récente que cette version d'Afterdusk ({supported})"
    )]
    TooNew { found: u32, supported: u32 },
    #[error("la migration {0} laisserait des liens rompus dans la table {1}")]
    ForeignKeys(u32, String),
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
    include_str!("migrations/0009_lieux_nommes.sql"),
    include_str!("migrations/0010_heure_du_boitier.sql"),
    include_str!("migrations/0011_fiches_materiel.sql"),
    include_str!("migrations/0012_entretien.sql"),
    include_str!("migrations/0013_rafales.sql"),
    include_str!("migrations/0014_analyse.sql"),
    include_str!("migrations/0015_clients.sql"),
    include_str!("migrations/0016_galeries.sql"),
    include_str!("migrations/0017_import_auto.sql"),
    include_str!("migrations/0018_dossiers_surveilles.sql"),
    include_str!("migrations/0019_videos.sql"),
    include_str!("migrations/0020_dossiers_releve.sql"),
    include_str!("migrations/0021_cadrages_livraison.sql"),
    include_str!("migrations/0022_compositions.sql"),
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
    if current == SCHEMA_VERSION {
        return Ok(());
    }
    // Une table recréée pour changer ses contraintes (méthode de SQLite :
    // créer, copier, supprimer, renommer) ne doit pas déclencher les actions
    // des clés étrangères (ON DELETE SET NULL vers elle) : elles sont coupées
    // le temps des migrations, hors transaction, et vérifiées avant de repartir.
    conn.pragma_update(None, "foreign_keys", false)?;
    let result = (|| -> Result<(), DbError> {
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            let broken: Option<String> = tx
                .query_row(
                    "SELECT \"table\" FROM pragma_foreign_key_check LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(table) = broken {
                return Err(DbError::ForeignKeys(index as u32 + 1, table));
            }
            tx.pragma_update(None, "user_version", index as u32 + 1)?;
            tx.commit()?;
        }
        Ok(())
    })();
    conn.pragma_update(None, "foreign_keys", true)?;
    result
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
    fn table_recreee_sans_rompre_les_liens() {
        // Base arrêtée avant la migration 0011, avec une photo liée à son
        // boîtier : le lien survit à la reconstruction de la table `gear`.
        let mut conn = Connection::open_in_memory().unwrap();
        configure(&conn).unwrap();
        for sql in &MIGRATIONS[..10] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 10).unwrap();
        conn.execute_batch(
            "INSERT INTO libraries (id, name, root_path, created_at) VALUES (1, 'P', '/p', 'x');
             INSERT INTO albums (id, library_id, name, folder, created_at)
               VALUES (1, 1, 'A', 'a', 'x');
             INSERT INTO gear (id, kind, make, model, created_at)
               VALUES (7, 'body', 'Nikon', 'Z 6', 'x');
             INSERT INTO gear_aliases (gear_id, exif_make, exif_model) VALUES (7, 'NIKON', 'Z 6');
             INSERT INTO media (id, library_id, album_id, kind, path, original_name, file_size,
                                fingerprint, content_hash, camera_id, imported_at, updated_at)
               VALUES (3, 1, 1, 'photo', 'a/1.NEF', '1.NEF', 1, 'f', 'h', 7, 'x', 'x');",
        )
        .unwrap();
        migrate(&mut conn).unwrap();
        let camera: Option<i64> = conn
            .query_row("SELECT camera_id FROM media WHERE id = 3", [], |r| r.get(0))
            .unwrap();
        assert_eq!(camera, Some(7));
        let aliases: i64 = conn
            .query_row(
                "SELECT count(*) FROM gear_aliases WHERE gear_id = 7",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(aliases, 1);
        // Les nouveaux types sont admis, les clés étrangères de nouveau actives.
        conn.execute(
            "INSERT INTO gear (kind, make, model, created_at) VALUES ('tripod', '', 'T', 'x')",
            [],
        )
        .unwrap();
        let fk: bool = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        assert!(fk);
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
