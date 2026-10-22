//! Base perdue puis reconstruite par la réanalyse de la bibliothèque :
//! photos, albums, marques, mots-clés, retouches, instantanés et copies
//! virtuelles repris des dossiers et des XMP, sans rien écrire sur le disque.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use afterdusk_lib::develop::settings::DevelopSettings;
use afterdusk_lib::develop::store::{self, ORIGINAL};
use afterdusk_lib::import::rescan::rescan;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::marks::{Mark, apply_mark, flush_pending_xmp};
use afterdusk_lib::{db, library};

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, String> {
    let mut out = BTreeMap::new();
    for e in walkdir::WalkDir::new(dir).into_iter().flatten() {
        if e.file_type().is_file() {
            out.insert(e.path().to_path_buf(), common::sha256(e.path()));
        }
    }
    out
}

#[test]
fn base_reconstruite_depuis_les_dossiers_et_les_xmp() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in ["nikon_z6.NEF", "sony_ilce-7m3.ARW"] {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let first = dir.path().join("premiere.db");
    let mut conn = db::open(&first).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let mut files: Vec<_> = fs::read_dir(&card)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    let id = import::plan(
        &mut conn,
        &ImportOptions {
            library_id: lib.id,
            source_path: card.clone(),
            source_label: "Carte".into(),
            files,
            album_name: "Sortie".into(),
            album_per_day: false,
            rename: false,
            backup: false,
            keywords: vec!["parc".into()],
            creator: Some("Lewis Bedar".into()),
            rights: None,
            skip_pair_jpegs: false,
            client_id: None,
            session_type_id: None,
            session_label: String::new(),
        },
    )
    .unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let nikon: i64 = conn
        .query_row("SELECT id FROM media WHERE path LIKE '%NEF'", [], |r| {
            r.get(0)
        })
        .unwrap();
    apply_mark(&mut conn, &[nikon], Mark::Rating(4)).unwrap();
    let mut s = DevelopSettings::default();
    s.light.exposure = 0.7;
    s.geometry.angle = 1.5;
    store::save(&mut conn, nikon, ORIGINAL, s, "light.exposure").unwrap();
    store::create_snapshot(&mut conn, nikon, ORIGINAL, "Avant contraste").unwrap();
    store::create_version(&mut conn, nikon, ORIGINAL, "Noir et blanc").unwrap();
    let mut bw = DevelopSettings::default();
    bw.color.saturation = -100.0;
    store::save(&mut conn, nikon, "Noir et blanc", bw, "color.saturation").unwrap();
    store::create_snapshot(&mut conn, nikon, "Noir et blanc", "Doux").unwrap();
    flush_pending_xmp(&conn, usize::MAX).unwrap();
    drop(conn);

    // Base perdue : une nouvelle, sur le même dossier.
    let before = snapshot(&root);
    let second = dir.path().join("nouvelle.db");
    let mut conn = db::open(&second).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let mut steps = Vec::new();
    let report = rescan(&mut conn, lib.id, &AtomicBool::new(false), |done, total| {
        steps.push((done, total))
    })
    .unwrap();
    assert_eq!(
        (report.added, report.restored),
        (2, 1),
        "{:?}",
        report.failed
    );
    assert_eq!(steps.last(), Some(&(2, 2)));
    // Règle absolue : rien n'a été écrit, ni original ni XMP.
    assert_eq!(snapshot(&root), before);

    let nikon: i64 = conn
        .query_row("SELECT id FROM media WHERE path LIKE '%NEF'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let (rating, album): (i64, String) = conn
        .query_row(
            "SELECT m.rating, a.name FROM media m JOIN albums a ON a.id = m.album_id WHERE m.id = ?1",
            [nikon],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(rating, 4);
    assert!(album.contains("Sortie"), "{album}");
    let keywords: i64 = conn
        .query_row(
            "SELECT count(*) FROM media_keywords WHERE media_id = ?1",
            [nikon],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(keywords, 1);
    let restored = store::load(&conn, nikon, ORIGINAL).unwrap();
    assert_eq!(
        (restored.light.exposure, restored.geometry.angle),
        (0.7, 1.5)
    );
    let snaps: Vec<String> = store::snapshots(&conn, nikon, ORIGINAL)
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(snaps, ["Avant contraste"]);
    let versions: Vec<String> = store::versions(&conn, nikon)
        .unwrap()
        .into_iter()
        .map(|v| v.name)
        .collect();
    assert_eq!(versions, ["Noir et blanc"]);
    assert_eq!(
        store::load(&conn, nikon, "Noir et blanc")
            .unwrap()
            .color
            .saturation,
        -100.0
    );
    assert_eq!(
        store::snapshots(&conn, nikon, "Noir et blanc").unwrap()[0].name,
        "Doux"
    );
    let history = store::history_of(&conn, nikon, ORIGINAL).unwrap();
    assert_eq!(
        history.steps.last().map(|s| s.label.as_str()),
        Some("xmp.restore")
    );

    // Une deuxième réanalyse n'ajoute rien.
    let again = rescan(&mut conn, lib.id, &AtomicBool::new(false), |_, _| {}).unwrap();
    assert_eq!(again.added, 0);
}
