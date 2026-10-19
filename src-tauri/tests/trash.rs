//! Corbeille interne : déplacement, restauration, vidage, purge et règles absolues.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use afterdusk_lib::db;
use afterdusk_lib::import::{self, ImportOptions, copy::hash_file, scan};
use afterdusk_lib::library::{self, paths, trash};
use afterdusk_lib::metadata::xmp;
use rusqlite::Connection;

struct Setup {
    _dir: tempfile::TempDir,
    root: PathBuf,
    backup: PathBuf,
    cache: PathBuf,
    conn: Connection,
    ids: Vec<i64>,
}

/// Bibliothèque de trois photos importées, avec sauvegarde.
fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for i in 0..3u8 {
        fs::write(card.join(format!("IMG_000{i}.RAF")), vec![i; 50_000]).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    let backup = dir.path().join("Sauvegarde");
    let cache = dir.path().join("cache");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&backup).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, Some(&backup)).unwrap();
    let files = scan::scan(&card)
        .unwrap()
        .files
        .into_iter()
        .map(|f| f.path)
        .collect();
    let opts = ImportOptions {
        library_id: lib.id,
        source_path: card,
        source_label: "Carte".into(),
        files,
        album_name: "Sortie".into(),
        album_per_day: false,
        rename: false,
        backup: true,
        keywords: vec![],
        creator: None,
        rights: None,
        skip_pair_jpegs: false,
    };
    let id = import::plan(&mut conn, &opts).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let ids = conn
        .prepare("SELECT id FROM media ORDER BY path")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    Setup {
        _dir: dir,
        root,
        backup,
        cache,
        conn,
        ids,
    }
}

fn path_of(conn: &Connection, id: i64) -> (String, String, String) {
    conn.query_row(
        "SELECT path, coalesce(sidecar_path, ''), state FROM media WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

fn all_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries {
        let p = entry.unwrap().path();
        if p.is_dir() {
            out.extend(all_files(&p));
        } else {
            out.push(p);
        }
    }
    out.sort();
    out
}

#[test]
fn corbeille_puis_restauration_a_l_identique() {
    let mut s = setup();
    let id = s.ids[0];
    let (path, sidecar, _) = path_of(&s.conn, id);
    let original = paths::to_absolute(&s.root, &path).unwrap();
    let hash = hash_file(&original).unwrap();

    let r = trash::move_to_trash(&mut s.conn, &[id]).unwrap();
    assert_eq!((r.done, r.failed), (1, 0));
    let (trashed, trashed_side, state) = path_of(&s.conn, id);
    assert_eq!(state, "trashed");
    assert!(trashed.starts_with(trash::TRASH_DIR));
    assert!(!original.exists());
    assert!(!paths::to_absolute(&s.root, &sidecar).unwrap().exists());
    let moved = paths::to_absolute(&s.root, &trashed).unwrap();
    assert_eq!(hash_file(&moved).unwrap(), hash);
    assert!(paths::to_absolute(&s.root, &trashed_side).unwrap().exists());

    let r = trash::restore(&mut s.conn, &[id]).unwrap();
    assert_eq!(r.done, 1);
    let (back, back_side, state) = path_of(&s.conn, id);
    assert_eq!(
        (back.as_str(), back_side.as_str(), state.as_str()),
        (path.as_str(), sidecar.as_str(), "normal")
    );
    assert_eq!(hash_file(&original).unwrap(), hash);
    // Le XMP suit la photo.
    assert!(
        xmp::read_sidecar(&paths::to_absolute(&s.root, &sidecar).unwrap())
            .unwrap()
            .is_some()
    );
    // Plus rien dans la corbeille.
    assert!(all_files(&s.root.join(trash::TRASH_DIR)).is_empty());
}

#[test]
fn restauration_sans_ecraser_un_fichier_arrive_depuis() {
    let mut s = setup();
    let id = s.ids[0];
    let (path, _, _) = path_of(&s.conn, id);
    trash::move_to_trash(&mut s.conn, &[id]).unwrap();
    let original = paths::to_absolute(&s.root, &path).unwrap();
    fs::write(&original, b"autre fichier").unwrap();

    trash::restore(&mut s.conn, &[id]).unwrap();
    let (restored, _, _) = path_of(&s.conn, id);
    assert_ne!(restored, path);
    assert!(restored.ends_with("-2.RAF"), "{restored}");
    assert_eq!(fs::read(&original).unwrap(), b"autre fichier");
}

/// Règle absolue : le vidage ne supprime que la corbeille, jamais la sauvegarde
/// ni les photos qui n'y sont pas.
#[test]
fn vidage_limite_a_la_corbeille() {
    let mut s = setup();
    let backup_before: Vec<_> = all_files(&s.backup)
        .into_iter()
        .map(|p| hash_file(&p).unwrap())
        .collect();
    assert_eq!(backup_before.len(), 3);
    let keep = s.ids[1];
    let (keep_path, _, _) = path_of(&s.conn, keep);

    trash::move_to_trash(&mut s.conn, &[s.ids[0]]).unwrap();
    // On demande aussi la purge d'une photo hors corbeille : elle est ignorée.
    let r = trash::purge(&mut s.conn, &s.cache, &[s.ids[0], keep]).unwrap();
    assert_eq!(r.done, 1);

    let count: i64 = s
        .conn
        .query_row("SELECT count(*) FROM media", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);
    assert!(paths::to_absolute(&s.root, &keep_path).unwrap().exists());
    assert!(all_files(&s.root.join(trash::TRASH_DIR)).is_empty());
    let backup_after: Vec<_> = all_files(&s.backup)
        .into_iter()
        .map(|p| hash_file(&p).unwrap())
        .collect();
    assert_eq!(backup_after, backup_before);
}

#[test]
fn purge_automatique_apres_le_delai() {
    let mut s = setup();
    trash::move_to_trash(&mut s.conn, &[s.ids[0], s.ids[1]]).unwrap();
    // L'une a été jetée il y a longtemps, l'autre aujourd'hui.
    s.conn
        .execute(
            "UPDATE media SET trashed_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
            [s.ids[0]],
        )
        .unwrap();
    assert_eq!(trash::retention_days(&s.conn).unwrap(), 30);
    let r = trash::purge_expired(&mut s.conn, &s.cache, 30).unwrap();
    assert_eq!(r.done, 1);
    assert_eq!(
        trash::trashed_ids(&s.conn, None, None).unwrap(),
        vec![s.ids[1]]
    );

    trash::set_retention_days(&s.conn, 7).unwrap();
    assert_eq!(trash::retention_days(&s.conn).unwrap(), 7);
}

#[test]
fn bibliotheque_debranchee_rien_ne_bouge() {
    let mut s = setup();
    trash::move_to_trash(&mut s.conn, &[s.ids[0]]).unwrap();
    let moved = s.root.with_file_name("debranchee");
    fs::rename(&s.root, &moved).unwrap();
    assert!(trash::restore(&mut s.conn, &[s.ids[0]]).is_err());
    assert!(trash::move_to_trash(&mut s.conn, &[s.ids[1]]).is_err());
    // La purge automatique ignore une bibliothèque débranchée.
    s.conn
        .execute("UPDATE media SET trashed_at = '2000-01-01T00:00:00Z'", [])
        .unwrap();
    assert_eq!(
        trash::purge_expired(&mut s.conn, &s.cache, 30)
            .unwrap()
            .done,
        0
    );
    fs::rename(&moved, &s.root).unwrap();
}
