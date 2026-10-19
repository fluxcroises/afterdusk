//! Bibliothèque sur un disque débranché, consultable grâce au cache
//! (cahier des charges §4.2) : on parcourt et on note ; au retour du disque,
//! les XMP en attente sont écrits.

mod common;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::db;
use afterdusk_lib::import::copy::hash_file;
use afterdusk_lib::import::{self, ImportError, ImportOptions};
use afterdusk_lib::library::marks::{Mark, apply_mark, flush_pending_xmp};
use afterdusk_lib::library::media::{self, MediaQuery, list_media};
use afterdusk_lib::library::{self, paths, trash};
use afterdusk_lib::metadata::xmp;
use rusqlite::Connection;

struct Setup {
    _dir: tempfile::TempDir,
    root: PathBuf,
    unplugged: PathBuf,
    card: PathBuf,
    cache: PathBuf,
    conn: Connection,
    library_id: i64,
}

fn options(s: &Setup, files: Vec<PathBuf>) -> ImportOptions {
    ImportOptions {
        library_id: s.library_id,
        source_path: s.card.clone(),
        source_label: "Carte".into(),
        files,
        album_name: "Sortie".into(),
        album_per_day: false,
        rename: false,
        backup: false,
        keywords: vec![],
        creator: None,
        rights: None,
        skip_pair_jpegs: false,
    }
}

/// Deux photos importées sur le « disque externe », aperçus en cache.
fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in ["fujifilm_x-t3.RAF", "canon_eos_r6.CR3"] {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.path().join("DisqueExterne").join("Photos");
    let unplugged = dir.path().join("DisqueExterne-debranche");
    fs::create_dir_all(&root).unwrap();
    let cache = dir.path().join("cache");
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let mut s = Setup {
        _dir: dir,
        root,
        unplugged,
        card: card.clone(),
        cache,
        conn: db::open_in_memory().unwrap(),
        library_id: lib.id,
    };
    let files = vec![
        card.join("fujifilm_x-t3.RAF"),
        card.join("canon_eos_r6.CR3"),
    ];
    let id = import::plan(&mut conn, &options(&s, files)).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM media")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for id in ids {
        media::generate_previews(&conn, &s.cache, id).unwrap();
    }
    s.conn = conn;
    s
}

/// Débranche le disque : sa racine disparaît (renommée hors de vue).
fn unplug(s: &Setup) {
    fs::rename(s.root.parent().unwrap(), &s.unplugged).unwrap();
}

fn replug(s: &Setup) {
    fs::rename(&s.unplugged, s.root.parent().unwrap()).unwrap();
}

#[test]
fn bibliotheque_debranchee_consultable_puis_rattrapee() {
    let mut s = setup();
    let originals: Vec<(String, String)> = s
        .conn
        .prepare("SELECT path, coalesce(sidecar_path, '') FROM media ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let hashes: Vec<String> = originals
        .iter()
        .map(|(p, _)| hash_file(&paths::to_absolute(&s.root, p).unwrap()).unwrap())
        .collect();

    unplug(&s);
    let lib = library::get_library(&s.conn, s.library_id).unwrap();
    assert!(!lib.is_online);
    library::record_presence(&s.conn, s.library_id, false).unwrap();

    // Parcourir : la grille, les aperçus en cache, la fiche.
    let list = list_media(&s.conn, s.library_id, &MediaQuery::default()).unwrap();
    assert_eq!(list.len(), 2);
    for m in &list {
        assert!(m.has_preview);
        for size in [256, 1024, 2048] {
            assert!(media::cached_preview(&s.cache, m.id, size).is_some());
        }
        assert!(media::media_details(&s.conn, m.id).unwrap().is_some());
        // Zoom 100 % : pas d'original, l'interface garde l'aperçu 2048 px.
        assert!(media::full_size_jpeg(&s.conn, m.id).unwrap().is_none());
    }

    // Noter : en base tout de suite, le XMP attend le disque.
    let first = list[0].id;
    apply_mark(&mut s.conn, &[first], Mark::Rating(5)).unwrap();
    assert_eq!(library::pending_xmp(&s.conn, s.library_id).unwrap(), 1);
    let report = flush_pending_xmp(&s.conn, 100).unwrap();
    assert_eq!((report.written, report.offline), (0, 1));

    // Ce qui touche aux fichiers est refusé, sans rien abîmer.
    assert!(trash::move_to_trash(&mut s.conn, &[first]).is_err());
    let file = s.card.join("fujifilm_x-t3.RAF");
    let opts = options(&s, vec![file]);
    assert!(matches!(
        import::plan(&mut s.conn, &opts),
        Err(ImportError::LibraryOffline(_))
    ));

    // Retour du disque : le XMP rattrape la note, les originaux sont intacts.
    replug(&s);
    library::record_presence(&s.conn, s.library_id, true).unwrap();
    assert!(library::last_seen(&s.conn, s.library_id).unwrap().is_some());
    let report = flush_pending_xmp(&s.conn, 100).unwrap();
    assert_eq!(report.written, 1);
    assert_eq!(library::pending_xmp(&s.conn, s.library_id).unwrap(), 0);
    let sidecar = &originals
        .iter()
        .zip(&list)
        .find(|(_, m)| m.id == first)
        .map(|((_, sc), _)| sc.clone())
        .unwrap_or_default();
    let data = xmp::read_sidecar(&paths::to_absolute(&s.root, sidecar).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(data.rating, 5);
    for ((path, _), hash) in originals.iter().zip(&hashes) {
        assert_eq!(
            &hash_file(&paths::to_absolute(&s.root, path).unwrap()).unwrap(),
            hash
        );
    }
}
