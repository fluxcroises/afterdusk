//! Dossier surveillé de bout en bout (décision 0009) : dossiers refusés,
//! fichiers déjà présents ignorés, nouvelle photo importée dans son album en
//! moins de 5 secondes, doublon ignoré, dossier jamais modifié.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use afterdusk_lib::import::ImportError;
use afterdusk_lib::import::auto::AutoSettings;
use afterdusk_lib::import::watch::{self, Tracker};
use afterdusk_lib::library::media::{MediaQuery, list_albums, list_media};
use afterdusk_lib::{db, library};

fn hashes(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file())
        .map(|p| (p.clone(), common::sha256(&p)))
        .collect();
    out.sort();
    out
}

#[test]
fn dossier_surveille() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Bibliotheque");
    let drop_zone = dir.path().join("Tether");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&drop_zone).unwrap();
    let mut conn = db::open(&dir.path().join("a.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let settings = AutoSettings {
        keywords: vec!["studio".into()],
        ..Default::default()
    };

    // Refusés : absent, dans la bibliothèque, contenant la bibliothèque.
    let refused = |path: &Path| watch::add(&conn, path, lib.id, "Studio", &settings);
    assert!(matches!(
        refused(&dir.path().join("absent")),
        Err(ImportError::WatchFolder("missing", _))
    ));
    fs::create_dir_all(root.join("sous-dossier")).unwrap();
    assert!(matches!(
        refused(&root.join("sous-dossier")),
        Err(ImportError::WatchFolder("insideLibrary", _))
    ));
    assert!(matches!(
        refused(dir.path()),
        Err(ImportError::WatchFolder("insideLibrary", _))
    ));

    // Une photo déjà là avant l'activation est ignorée.
    let [old, new, ..] = common::RAW_SAMPLES;
    fs::copy(common::sample(old), drop_zone.join(old)).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let id = watch::add(&conn, &drop_zone, lib.id, "Séance studio", &settings).unwrap();
    assert!(matches!(
        watch::add(&conn, &drop_zone, lib.id, "Autre", &settings),
        Err(ImportError::WatchFolder("duplicate", _))
    ));
    let folder = watch::list(&conn).unwrap().remove(0);
    assert!(watch::arrivals(&folder).is_empty());

    // Le logiciel du boîtier dépose une nouvelle photo : chronomètre lancé.
    std::thread::sleep(Duration::from_millis(20));
    let start = Instant::now();
    fs::copy(common::sample(new), drop_zone.join(new)).unwrap();
    let before = hashes(&drop_zone);
    let mut tracker = Tracker::default();
    let ready = loop {
        let now = Instant::now();
        for path in watch::arrivals(&folder) {
            tracker.note(path, now);
        }
        let ready = tracker.ready(now);
        if !ready.is_empty() {
            break ready;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "fichier jamais prêt"
        );
        std::thread::sleep(Duration::from_millis(250));
    };
    assert_eq!(ready, vec![drop_zone.join(new)]);
    let imported = watch::import_files(&mut conn, &folder, ready, &AtomicBool::new(false), |_| {})
        .unwrap()
        .unwrap();
    let elapsed = start.elapsed();
    assert_eq!(imported.media_ids.len(), 1);
    assert!(
        elapsed < Duration::from_secs(5),
        "photo affichable après {elapsed:?}"
    );
    println!("dossier surveillé : photo importée en {elapsed:?}");
    let albums = list_albums(&conn, lib.id).unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].name, "Séance studio");
    assert_eq!(
        list_media(&conn, lib.id, &MediaQuery::default())
            .unwrap()
            .len(),
        1
    );

    // La même photo, revue à la relecture : déjà importée, rien ne se passe.
    let again = watch::import_files(
        &mut conn,
        &folder,
        vec![drop_zone.join(new)],
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert!(again.is_none());

    // Pause puis reprise : ce qui est arrivé entre-temps est ignoré.
    watch::set_enabled(&conn, id, false).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    watch::set_enabled(&conn, id, true).unwrap();
    let folder = watch::list(&conn).unwrap().remove(0);
    assert!(watch::arrivals(&folder).is_empty());

    // Règle absolue : le dossier surveillé n'a pas été modifié.
    assert_eq!(before, hashes(&drop_zone));
    watch::delete(&conn, id).unwrap();
    assert!(watch::list(&conn).unwrap().is_empty());
    assert_eq!(before, hashes(&drop_zone));
}
