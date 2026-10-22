//! Clients et types de séance (cahier des charges, 4.2 ; décision 0009).
//! Une fiche client (nom, courriel et téléphone facultatifs, notes) et un
//! type de séance se rattachent à un album. Les clients sont communs à
//! toutes les bibliothèques. Leurs données restent dans la base, jamais dans
//! les XMP ; supprimer un client efface sa fiche et le retire de ses albums,
//! sans toucher aux photos.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::LibraryError;
use crate::db::now_utc;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
    /// Albums rattachés, toutes bibliothèques confondues.
    pub album_count: i64,
}

/// Ce que l'utilisateur saisit dans la fiche.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientInput {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
}

/// Album d'un client, pour sa fiche.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAlbum {
    pub id: i64,
    pub library_id: i64,
    pub library_name: String,
    pub name: String,
    pub start_date: Option<String>,
    pub session_type_id: Option<i64>,
}

/// Type de séance : `key` pour un type d'origine (traduit par l'interface),
/// `name` pour un type ajouté.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionType {
    pub id: i64,
    pub key: Option<String>,
    pub name: Option<String>,
}

fn clean_name(name: &str) -> Result<String, LibraryError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LibraryError::EmptyName);
    }
    Ok(name.to_owned())
}

/// Champ facultatif : vide ou blanc devient absent.
fn optional(text: &Option<String>) -> Option<String> {
    text.as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// Clients, par nom.
pub fn list(conn: &Connection) -> Result<Vec<Client>, LibraryError> {
    let rows = conn
        .prepare(
            "SELECT c.id, c.name, c.email, c.phone, c.notes,
                    (SELECT count(*) FROM albums a WHERE a.client_id = c.id)
               FROM clients c ORDER BY c.name COLLATE NOCASE, c.id",
        )?
        .query_map([], |r| {
            Ok(Client {
                id: r.get(0)?,
                name: r.get(1)?,
                email: r.get(2)?,
                phone: r.get(3)?,
                notes: r.get(4)?,
                album_count: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn create(conn: &Connection, input: &ClientInput) -> Result<i64, LibraryError> {
    let name = clean_name(&input.name)?;
    conn.execute(
        "INSERT INTO clients (name, email, phone, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            name,
            optional(&input.email),
            optional(&input.phone),
            optional(&input.notes),
            now_utc()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, input: &ClientInput) -> Result<(), LibraryError> {
    let name = clean_name(&input.name)?;
    if conn.execute(
        "UPDATE clients SET name = ?2, email = ?3, phone = ?4, notes = ?5 WHERE id = ?1",
        params![
            id,
            name,
            optional(&input.email),
            optional(&input.phone),
            optional(&input.notes)
        ],
    )? == 0
    {
        return Err(LibraryError::ClientNotFound(id));
    }
    Ok(())
}

/// Efface la fiche ; ses albums restent, sans client.
pub fn delete(conn: &Connection, id: i64) -> Result<(), LibraryError> {
    if conn.execute("DELETE FROM clients WHERE id = ?1", [id])? == 0 {
        return Err(LibraryError::ClientNotFound(id));
    }
    Ok(())
}

pub fn name_of(conn: &Connection, id: i64) -> Result<String, LibraryError> {
    conn.query_row("SELECT name FROM clients WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .ok_or(LibraryError::ClientNotFound(id))
}

/// Albums d'un client, du plus récent au plus ancien.
pub fn albums(conn: &Connection, id: i64) -> Result<Vec<ClientAlbum>, LibraryError> {
    let rows = conn
        .prepare(
            "SELECT a.id, a.library_id, l.name, a.name, a.start_date, a.session_type_id
               FROM albums a JOIN libraries l ON l.id = a.library_id
              WHERE a.client_id = ?1
              ORDER BY a.start_date DESC, a.name",
        )?
        .query_map([id], |r| {
            Ok(ClientAlbum {
                id: r.get(0)?,
                library_id: r.get(1)?,
                library_name: r.get(2)?,
                name: r.get(3)?,
                start_date: r.get(4)?,
                session_type_id: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Rattache (ou détache, avec `None`) un client et un type de séance à un album.
pub fn set_album(
    conn: &Connection,
    album_id: i64,
    client_id: Option<i64>,
    session_type_id: Option<i64>,
) -> Result<(), LibraryError> {
    if let Some(id) = client_id {
        name_of(conn, id)?;
    }
    if let Some(id) = session_type_id {
        session_type(conn, id)?;
    }
    if conn.execute(
        "UPDATE albums SET client_id = ?2, session_type_id = ?3 WHERE id = ?1",
        params![album_id, client_id, session_type_id],
    )? == 0
    {
        return Err(LibraryError::NotFound(album_id));
    }
    Ok(())
}

/// Types de séance, dans l'ordre de la liste.
pub fn session_types(conn: &Connection) -> Result<Vec<SessionType>, LibraryError> {
    let rows = conn
        .prepare("SELECT id, key, name FROM session_types ORDER BY position, id")?
        .query_map([], |r| {
            Ok(SessionType {
                id: r.get(0)?,
                key: r.get(1)?,
                name: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

fn session_type(conn: &Connection, id: i64) -> Result<SessionType, LibraryError> {
    conn.query_row(
        "SELECT id, key, name FROM session_types WHERE id = ?1",
        [id],
        |r| {
            Ok(SessionType {
                id: r.get(0)?,
                key: r.get(1)?,
                name: r.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or(LibraryError::SessionTypeNotFound(id))
}

/// Nouveau type, en fin de liste.
pub fn create_session_type(conn: &Connection, name: &str) -> Result<i64, LibraryError> {
    let name = clean_name(name)?;
    conn.execute(
        "INSERT INTO session_types (name, position)
         VALUES (?1, (SELECT coalesce(max(position), 0) + 1 FROM session_types))",
        [name],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Renomme un type ; un type d'origine renommé garde le nom saisi, dans
/// toutes les langues.
pub fn rename_session_type(conn: &Connection, id: i64, name: &str) -> Result<(), LibraryError> {
    let name = clean_name(name)?;
    if conn.execute(
        "UPDATE session_types SET key = NULL, name = ?2 WHERE id = ?1",
        params![id, name],
    )? == 0
    {
        return Err(LibraryError::SessionTypeNotFound(id));
    }
    Ok(())
}

/// Retire un type ; ses albums restent, sans type.
pub fn delete_session_type(conn: &Connection, id: i64) -> Result<(), LibraryError> {
    if conn.execute("DELETE FROM session_types WHERE id = ?1", [id])? == 0 {
        return Err(LibraryError::SessionTypeNotFound(id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::library::{create_library, find_or_create_album};

    fn setup() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
        let root = dir.path().join("Photos");
        std::fs::create_dir(&root).unwrap();
        let lib = create_library(&conn, "Photos", &root, None).unwrap();
        let album = find_or_create_album(&conn, lib.id, "Mariage", "2026/Mariage", None).unwrap();
        (dir, conn, album)
    }

    fn input(name: &str, email: &str) -> ClientInput {
        ClientInput {
            name: name.into(),
            email: Some(email.into()),
            phone: Some("  ".into()),
            notes: None,
        }
    }

    #[test]
    fn fiche_client_et_album() {
        let (_dir, conn, album) = setup();
        assert!(matches!(
            create(&conn, &input("  ", "")),
            Err(LibraryError::EmptyName)
        ));
        let id = create(&conn, &input(" Famille Dupont ", "dupont@example.com")).unwrap();
        let wedding = session_types(&conn).unwrap()[0].id;
        set_album(&conn, album, Some(id), Some(wedding)).unwrap();
        let c = &list(&conn).unwrap()[0];
        assert_eq!(c.name, "Famille Dupont");
        assert_eq!(c.email.as_deref(), Some("dupont@example.com"));
        // Un champ blanc n'est pas gardé.
        assert_eq!((c.phone.clone(), c.album_count), (None, 1));
        let albums = albums(&conn, id).unwrap();
        assert_eq!(
            (albums[0].name.as_str(), albums[0].session_type_id),
            ("Mariage", Some(wedding))
        );
        update(&conn, id, &input("Dupont", "")).unwrap();
        assert_eq!(list(&conn).unwrap()[0].email, None);
        assert!(matches!(
            set_album(&conn, album, Some(99), None),
            Err(LibraryError::ClientNotFound(99))
        ));
    }

    /// Règle de la décision 0009 : supprimer un client efface sa fiche et le
    /// retire de ses albums ; l'album et ses photos restent.
    #[test]
    fn supprimer_un_client_efface_sa_fiche_seulement() {
        let (_dir, conn, album) = setup();
        let id = create(&conn, &input("Dupont", "dupont@example.com")).unwrap();
        set_album(&conn, album, Some(id), None).unwrap();
        delete(&conn, id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        let left: (i64, Option<i64>) = conn
            .query_row(
                "SELECT count(*), max(client_id) FROM albums WHERE id = ?1",
                [album],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(left, (1, None));
        let traces: i64 = conn
            .query_row(
                "SELECT count(*) FROM clients WHERE name LIKE '%Dupont%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(traces, 0);
        assert!(matches!(
            delete(&conn, id),
            Err(LibraryError::ClientNotFound(_))
        ));
    }

    #[test]
    fn types_de_seance_modifiables() {
        let (_dir, conn, album) = setup();
        let types = session_types(&conn).unwrap();
        assert_eq!(types.len(), 11);
        assert_eq!(types[0].key.as_deref(), Some("wedding"));
        assert_eq!(types[10].key.as_deref(), Some("concert"));
        let drone = create_session_type(&conn, "Drone").unwrap();
        let types = session_types(&conn).unwrap();
        assert_eq!(types.last().unwrap().name.as_deref(), Some("Drone"));
        rename_session_type(&conn, types[0].id, "Mariage civil").unwrap();
        let first = &session_types(&conn).unwrap()[0];
        assert_eq!(
            (first.key.clone(), first.name.as_deref()),
            (None, Some("Mariage civil"))
        );
        set_album(&conn, album, None, Some(drone)).unwrap();
        delete_session_type(&conn, drone).unwrap();
        let kept: Option<i64> = conn
            .query_row(
                "SELECT session_type_id FROM albums WHERE id = ?1",
                [album],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, None);
    }
}
