//! Réglages de retouche : base, historique et XMP (décision 0005).

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::db;
use afterdusk_lib::develop::settings::{DevelopSettings, WhiteBalance};
use afterdusk_lib::develop::store::{self, ORIGINAL};
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
    assert!(store::load(&conn, media_id, ORIGINAL).unwrap().is_neutral());
    assert_eq!(
        store::xmp_data(&conn, media_id).unwrap(),
        store::DevelopXmp::default()
    );

    let mut s = DevelopSettings::default();
    s.light.exposure = 0.7;
    store::save(&mut conn, media_id, ORIGINAL, s.clone(), "Exposition").unwrap();
    s.light.exposure = 9.0; // hors plage : ramenée à +5
    s.color.white_balance = WhiteBalance::Custom {
        temperature: 5600.0,
        tint: 3.0,
    };
    let saved = store::save(&mut conn, media_id, ORIGINAL, s, "Balance des blancs").unwrap();
    assert_eq!(saved.light.exposure, 5.0);
    assert_eq!(store::load(&conn, media_id, ORIGINAL).unwrap(), saved);
    let labels = |conn: &rusqlite::Connection| -> Vec<String> {
        store::history_of(conn, media_id, ORIGINAL)
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
        ORIGINAL,
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
    let h = store::history_of(&conn, media_id, ORIGINAL).unwrap();
    assert!(h.steps.is_empty() && h.current == 0);

    let mut s = DevelopSettings::default();
    for (label, exposure) in [("light.exposure", 0.5), ("light.exposure", 1.0)] {
        s.light.exposure = exposure;
        store::save(&mut conn, media_id, ORIGINAL, s.clone(), label).unwrap();
    }
    s.light.contrast = 30.0;
    store::save(&mut conn, media_id, ORIGINAL, s.clone(), "light.contrast").unwrap();
    assert_eq!(
        store::history_of(&conn, media_id, ORIGINAL)
            .unwrap()
            .current,
        3
    );

    // Annuler deux fois : l'état de la première étape, en base comme au XMP.
    let back = store::go_to(&mut conn, media_id, ORIGINAL, 1).unwrap();
    assert_eq!(back.light.exposure, 0.5);
    assert_eq!(back.light.contrast, 0.0);
    assert_eq!(store::load(&conn, media_id, ORIGINAL).unwrap(), back);
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
    let h = store::history_of(&conn, media_id, ORIGINAL).unwrap();
    assert_eq!((h.current, h.steps.len()), (1, 3));
    assert_eq!(store::go_to(&mut conn, media_id, ORIGINAL, 3).unwrap(), s);

    // Retour à l'original : rendu neutre, XMP sans réglages.
    assert!(
        store::go_to(&mut conn, media_id, ORIGINAL, 0)
            .unwrap()
            .is_neutral()
    );
    flush_pending_xmp(&conn, 10).unwrap();
    let xmp = xmp::read_sidecar(&paths::to_absolute(&root, &sidecar).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(xmp.develop, None);

    // Un nouveau réglage après une annulation remplace les étapes qui suivaient.
    store::go_to(&mut conn, media_id, ORIGINAL, 1).unwrap();
    let mut t = back.clone();
    t.color.saturation = -40.0;
    store::save(&mut conn, media_id, ORIGINAL, t.clone(), "color.saturation").unwrap();
    let h = store::history_of(&conn, media_id, ORIGINAL).unwrap();
    let labels: Vec<_> = h.steps.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["light.exposure", "color.saturation"]);
    assert_eq!(h.current, 2);
    assert_eq!(h.steps[1].settings, t);

    // Étape inexistante : refusée, rien ne change.
    assert!(matches!(
        store::go_to(&mut conn, media_id, ORIGINAL, 9),
        Err(store::StoreError::NoSuchStep(9))
    ));
    assert_eq!(store::load(&conn, media_id, ORIGINAL).unwrap(), t);
}

#[test]
fn copies_virtuelles_et_instantanes() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    fs::copy(common::sample("nikon_z6.NEF"), card.join("DSC_0001.NEF")).unwrap();
    let source_hash = common::sha256(&card.join("DSC_0001.NEF"));
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let opts = ImportOptions {
        library_id: lib.id,
        source_path: card.clone(),
        source_label: "Carte".into(),
        files: vec![card.join("DSC_0001.NEF")],
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
    let (media_id, sidecar, path): (i64, String, String) = conn
        .query_row("SELECT id, sidecar_path, path FROM media", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    let original = paths::to_absolute(&root, &path).unwrap();
    let original_hash = common::sha256(&original);

    // Instantané de l'originale, puis on continue à retoucher.
    let mut warm = DevelopSettings::default();
    warm.light.exposure = 0.4;
    store::save(
        &mut conn,
        media_id,
        ORIGINAL,
        warm.clone(),
        "light.exposure",
    )
    .unwrap();
    store::create_snapshot(&mut conn, media_id, ORIGINAL, "  Client  ").unwrap();
    assert!(matches!(
        store::create_snapshot(&mut conn, media_id, ORIGINAL, "Client"),
        Err(store::StoreError::NameTaken(_))
    ));
    assert!(matches!(
        store::create_snapshot(&mut conn, media_id, ORIGINAL, "   "),
        Err(store::StoreError::InvalidName)
    ));
    let mut later = warm.clone();
    later.color.saturation = -100.0;
    store::save(
        &mut conn,
        media_id,
        ORIGINAL,
        later.clone(),
        "color.saturation",
    )
    .unwrap();

    // Une annulation suivie d'un réglage ne fait pas disparaître l'instantané.
    store::go_to(&mut conn, media_id, ORIGINAL, 1).unwrap();
    store::save(
        &mut conn,
        media_id,
        ORIGINAL,
        later.clone(),
        "color.saturation",
    )
    .unwrap();
    let snaps = store::snapshots(&conn, media_id, ORIGINAL).unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(
        (snaps[0].name.as_str(), &snaps[0].settings),
        ("Client", &warm)
    );

    // Appliquer l'instantané : une étape d'historique, annulable.
    let applied = store::apply_snapshot(&mut conn, media_id, ORIGINAL, "Client").unwrap();
    assert_eq!(applied, warm);
    let h = store::history_of(&conn, media_id, ORIGINAL).unwrap();
    assert_eq!(h.steps.last().unwrap().label, "snapshot:Client");
    let undone = store::go_to(&mut conn, media_id, ORIGINAL, h.current - 1).unwrap();
    assert_eq!(undone, later);

    // Copie virtuelle : part des réglages de l'originale, vit sa vie.
    store::create_version(&mut conn, media_id, ORIGINAL, "N&B").unwrap();
    assert!(matches!(
        store::create_version(&mut conn, media_id, ORIGINAL, "N&B"),
        Err(store::StoreError::NameTaken(_))
    ));
    assert_eq!(store::load(&conn, media_id, "N&B").unwrap(), later);
    let copy_history = store::history_of(&conn, media_id, "N&B").unwrap();
    assert_eq!(copy_history.current, 1);
    assert_eq!(copy_history.steps[0].label, store::VERSION_CREATED);
    let mut bw = later.clone();
    bw.light.contrast = 40.0;
    store::save(&mut conn, media_id, "N&B", bw.clone(), "light.contrast").unwrap();
    store::create_snapshot(&mut conn, media_id, "N&B", "Fort").unwrap();
    // L'originale n'a pas bougé.
    assert_eq!(store::load(&conn, media_id, ORIGINAL).unwrap(), later);

    store::rename_version(&mut conn, media_id, "N&B", "Noir et blanc").unwrap();
    let names: Vec<_> = store::versions(&conn, media_id)
        .unwrap()
        .into_iter()
        .map(|v| v.name)
        .collect();
    assert_eq!(names, ["Noir et blanc"]);
    assert!(store::load(&conn, media_id, "N&B").is_err());

    // Le XMP garde l'originale, ses instantanés et les copies.
    flush_pending_xmp(&conn, 10).unwrap();
    let xmp_path = paths::to_absolute(&root, &sidecar).unwrap();
    let data = xmp::read_sidecar(&xmp_path).unwrap().unwrap();
    assert_eq!(
        DevelopSettings::from_json(&data.develop.unwrap()).unwrap(),
        later
    );
    let snaps: serde_json::Value = serde_json::from_str(&data.snapshots.unwrap()).unwrap();
    assert_eq!(snaps[0]["name"], "Client");
    let versions: serde_json::Value = serde_json::from_str(&data.versions.unwrap()).unwrap();
    assert_eq!(versions[0]["name"], "Noir et blanc");
    assert_eq!(versions[0]["snapshots"][0]["name"], "Fort");
    let copy_settings = DevelopSettings::from_json(&versions[0]["settings"].to_string()).unwrap();
    assert_eq!(copy_settings, bw);

    // Supprimer la copie et l'instantané : retirés de la base et du XMP.
    store::delete_version(&mut conn, media_id, "Noir et blanc").unwrap();
    store::delete_snapshot(&mut conn, media_id, ORIGINAL, "Client").unwrap();
    assert!(store::delete_version(&mut conn, media_id, ORIGINAL).is_err());
    flush_pending_xmp(&conn, 10).unwrap();
    let data = xmp::read_sidecar(&xmp_path).unwrap().unwrap();
    assert_eq!((data.snapshots, data.versions), (None, None));
    assert!(data.develop.is_some());

    // Ni l'original ni la carte n'ont été touchés.
    assert_eq!(common::sha256(&original), original_hash);
    assert_eq!(common::sha256(&card.join("DSC_0001.NEF")), source_hash);
}
