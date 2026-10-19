//! Réglages de retouche : base, historique et XMP (décision 0005).

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::db;
use afterdusk_lib::develop::settings::{DevelopSettings, WhiteBalance};
use afterdusk_lib::develop::store;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::marks::{Mark, apply_mark, flush_pending_xmp};
use afterdusk_lib::library::{self, paths};
use afterdusk_lib::metadata::xmp;

#[test]
fn reglages_en_base_dans_l_historique_et_le_xmp() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    fs::copy(
        common::sample("fujifilm_x-t3.RAF"),
        card.join("DSCF0001.RAF"),
    )
    .unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let opts = ImportOptions {
        library_id: lib.id,
        source_path: card.clone(),
        source_label: "Carte".into(),
        files: vec![card.join("DSCF0001.RAF")],
        album_name: "Sortie".into(),
        album_per_day: false,
        rename: false,
        backup: false,
        keywords: vec!["mer".into()],
        creator: None,
        rights: None,
        skip_pair_jpegs: false,
    };
    let id = import::plan(&mut conn, &opts).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let (media_id, sidecar): (i64, String) = conn
        .query_row("SELECT id, sidecar_path FROM media", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    let xmp_path = paths::to_absolute(&root, &sidecar).unwrap();

    // Jamais retouchée : rendu neutre, rien dans le XMP.
    assert!(store::load(&conn, media_id).unwrap().is_neutral());
    assert_eq!(store::xmp_json(&conn, media_id).unwrap(), None);

    let mut s = DevelopSettings::default();
    s.light.exposure = 0.7;
    store::save(&mut conn, media_id, s.clone(), "Exposition").unwrap();
    s.light.exposure = 9.0; // hors plage : ramenée à +5
    s.color.white_balance = WhiteBalance::Custom {
        temperature: 5600.0,
        tint: 3.0,
    };
    let saved = store::save(&mut conn, media_id, s, "Balance des blancs").unwrap();
    assert_eq!(saved.light.exposure, 5.0);
    assert_eq!(store::load(&conn, media_id).unwrap(), saved);
    let labels = |conn: &rusqlite::Connection| -> Vec<String> {
        store::history_of(conn, media_id)
            .unwrap()
            .steps
            .into_iter()
            .map(|s| s.label)
            .collect()
    };
    assert_eq!(labels(&conn), ["Exposition", "Balance des blancs"]);

    // Le XMP reçoit les réglages avec les marques, sans perdre le reste.
    apply_mark(&mut conn, &[media_id], Mark::Rating(4)).unwrap();
    flush_pending_xmp(&conn, 10).unwrap();
    let data = xmp::read_sidecar(&xmp_path).unwrap().unwrap();
    assert_eq!(data.rating, 4);
    assert_eq!(data.keywords, ["mer"]);
    let develop = data.develop.expect("réglages dans le XMP");
    assert_eq!(DevelopSettings::from_json(&develop).unwrap(), saved);

    // Retour au neutre : la propriété disparaît du XMP.
    store::save(
        &mut conn,
        media_id,
        DevelopSettings::default(),
        "Réinitialiser",
    )
    .unwrap();
    flush_pending_xmp(&conn, 10).unwrap();
    let data = xmp::read_sidecar(&xmp_path).unwrap().unwrap();
    assert_eq!(data.develop, None);
    assert_eq!(data.rating, 4);
}

#[test]
fn annuler_retablir_et_revenir_a_une_etape() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    fs::copy(
        common::sample("sony_ilce-7m3.ARW"),
        card.join("DSC0001.ARW"),
    )
    .unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let opts = ImportOptions {
        library_id: lib.id,
        source_path: card.clone(),
        source_label: "Carte".into(),
        files: vec![card.join("DSC0001.ARW")],
        album_name: "Sortie".into(),
        album_per_day: false,
        rename: false,
        backup: false,
        keywords: vec![],
        creator: None,
        rights: None,
        skip_pair_jpegs: false,
    };
    let id = import::plan(&mut conn, &opts).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let media_id: i64 = conn
        .query_row("SELECT id FROM media", [], |r| r.get(0))
        .unwrap();

    // Jamais retouchée : historique vide, position à l'original.
    let h = store::history_of(&conn, media_id).unwrap();
    assert!(h.steps.is_empty() && h.current == 0);

    let mut s = DevelopSettings::default();
    for (label, exposure) in [("light.exposure", 0.5), ("light.exposure", 1.0)] {
        s.light.exposure = exposure;
        store::save(&mut conn, media_id, s.clone(), label).unwrap();
    }
    s.light.contrast = 30.0;
    store::save(&mut conn, media_id, s.clone(), "light.contrast").unwrap();
    assert_eq!(store::history_of(&conn, media_id).unwrap().current, 3);

    // Annuler deux fois : l'état de la première étape, en base comme au XMP.
    let back = store::go_to(&mut conn, media_id, 1).unwrap();
    assert_eq!(back.light.exposure, 0.5);
    assert_eq!(back.light.contrast, 0.0);
    assert_eq!(store::load(&conn, media_id).unwrap(), back);
    flush_pending_xmp(&conn, 10).unwrap();
    let sidecar: String = conn
        .query_row("SELECT sidecar_path FROM media", [], |r| r.get(0))
        .unwrap();
    let xmp = xmp::read_sidecar(&paths::to_absolute(&root, &sidecar).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        DevelopSettings::from_json(&xmp.develop.unwrap()).unwrap(),
        back
    );

    // Rétablir : les étapes annulées sont toujours là.
    let h = store::history_of(&conn, media_id).unwrap();
    assert_eq!((h.current, h.steps.len()), (1, 3));
    assert_eq!(store::go_to(&mut conn, media_id, 3).unwrap(), s);

    // Retour à l'original : rendu neutre, XMP sans réglages.
    assert!(store::go_to(&mut conn, media_id, 0).unwrap().is_neutral());
    flush_pending_xmp(&conn, 10).unwrap();
    let xmp = xmp::read_sidecar(&paths::to_absolute(&root, &sidecar).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(xmp.develop, None);

    // Un nouveau réglage après une annulation remplace les étapes qui suivaient.
    store::go_to(&mut conn, media_id, 1).unwrap();
    let mut t = back.clone();
    t.color.saturation = -40.0;
    store::save(&mut conn, media_id, t.clone(), "color.saturation").unwrap();
    let h = store::history_of(&conn, media_id).unwrap();
    let labels: Vec<_> = h.steps.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["light.exposure", "color.saturation"]);
    assert_eq!(h.current, 2);
    assert_eq!(h.steps[1].settings, t);

    // Étape inexistante : refusée, rien ne change.
    assert!(matches!(
        store::go_to(&mut conn, media_id, 9),
        Err(store::StoreError::NoSuchStep(9))
    ));
    assert_eq!(store::load(&conn, media_id).unwrap(), t);
}
