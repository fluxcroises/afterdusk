//! Détection des ratés de bout en bout : les six RAW de test sont nets et
//! bien exposés ; une photo floutée et une photo brûlée sont signalées, et le
//! filtre « Ratés probables » ne garde qu'elles. Une seconde analyse ne
//! refait rien. Les originaux ne sont que lus.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::analysis::run;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::filter::MediaFilter;
use afterdusk_lib::library::media::{MediaQuery, list_media};
use afterdusk_lib::metadata::media::MediaFormat;
use afterdusk_lib::raw::preview;
use afterdusk_lib::{db, library};

#[test]
fn rates_signales_et_filtres() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in common::RAW_SAMPLES {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    // Une prise floue (bougé simulé) et une prise brûlée, en JPEG.
    let img = preview::load(&common::sample("nikon_z6.NEF"), MediaFormat::Raw, 1, 1024).unwrap();
    image::imageops::blur(&img, 4.0)
        .save(card.join("floue.jpg"))
        .unwrap();
    image::RgbImage::from_fn(800, 600, |x, _| image::Rgb([250, 252, (x % 6) as u8 + 249]))
        .save(card.join("brulee.jpg"))
        .unwrap();
    let before = common::sha256(&card.join("floue.jpg"));

    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let cache = dir.path().join("cache");
    let mut conn = db::open(&dir.path().join("a.db")).unwrap();
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
            album_name: "Ratés".into(),
            album_per_day: false,
            rename: false,
            backup: false,
            keywords: vec![],
            creator: None,
            rights: None,
            skip_pair_jpegs: false,
        },
    )
    .unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();

    let mut calls = 0;
    let report = run::run(
        &conn,
        &cache,
        lib.id,
        None,
        &AtomicBool::new(false),
        |_, _| calls += 1,
    )
    .unwrap();
    assert_eq!((report.analyzed, report.misses, report.skipped), (8, 2, 0));
    assert_eq!(calls, 8);

    // Chaque photo porte ses raisons ; les autres n'en ont aucune.
    let all = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    let misses = |name: &str| {
        all.iter()
            .find(|m| m.file_name == name)
            .unwrap()
            .misses
            .clone()
    };
    assert_eq!(misses("floue.jpg"), "blur");
    assert_eq!(
        misses("brulee.jpg"),
        "blur over",
        "un aplat brûlé n'a aucun détail"
    );
    assert!(
        all.iter()
            .filter(|m| !m.file_name.ends_with(".jpg"))
            .all(|m| m.misses.is_empty())
    );

    // Le filtre « Ratés probables » ne garde qu'elles.
    let q = MediaQuery {
        filter: MediaFilter {
            misses: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut names: Vec<String> = list_media(&conn, lib.id, &q)
        .unwrap()
        .into_iter()
        .map(|m| m.file_name)
        .collect();
    names.sort();
    assert_eq!(names, ["brulee.jpg", "floue.jpg"]);

    // Déjà analysées par cette version : rien à refaire.
    let again = run::run(
        &conn,
        &cache,
        lib.id,
        None,
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    assert_eq!(again.analyzed, 0);
    // La source n'a pas bougé.
    assert_eq!(common::sha256(&card.join("floue.jpg")), before);
}
