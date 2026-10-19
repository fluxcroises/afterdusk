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
    assert_eq!(
        store::history(&conn, media_id).unwrap(),
        ["Exposition", "Balance des blancs"]
    );

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
