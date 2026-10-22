//! Matériel de bout en bout : l'inventaire se remplit à l'import des six RAW
//! de test, puis fusion, masquage, accessoire, fiche et photo déposée. Une
//! photo déposée est copiée : sa source ne change pas.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::gear::maintenance::{self, MaintenanceInput, MaintenanceKind};
use afterdusk_lib::gear::{self, GearError, GearKind, GearUpdate};
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::{db, library};

#[test]
fn inventaire_fusion_masquage_accessoires_et_fiche() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in common::RAW_SAMPLES {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
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
            album_name: "Matériel".into(),
            album_per_day: false,
            rename: false,
            backup: false,
            keywords: vec![],
            creator: None,
            rights: None,
            skip_pair_jpegs: false,
            client_id: None,
            session_type_id: None,
            session_label: String::new(),
        },
    )
    .unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();

    // Inventaire : un boîtier par RAW, chacun avec sa photo.
    let items = gear::list(&conn, false).unwrap();
    let bodies: Vec<_> = items.iter().filter(|g| g.kind == GearKind::Body).collect();
    assert_eq!(bodies.len(), 6, "{items:#?}");
    assert!(bodies.iter().all(|b| b.photo_count == 1 && b.auto_detected));
    assert!(items.iter().any(|g| g.kind == GearKind::Lens));
    let nikon = bodies.iter().find(|b| b.model.contains("Z 6")).unwrap().id;
    let sony = bodies.iter().find(|b| b.model.contains("7M3")).unwrap().id;
    assert_eq!(
        bodies
            .iter()
            .find(|b| b.id == nikon)
            .unwrap()
            .style
            .as_deref(),
        Some("mirrorless")
    );

    // Statistiques : compteur lu chez Nikon et Fujifilm, estimé ailleurs.
    // Compteur effacé, comme pour une photo importée avant sa lecture : il
    // est relevé dans l'original à l'ouverture des statistiques.
    conn.execute("UPDATE media_exif SET shutter_count = NULL", [])
        .unwrap();
    let stats = gear::stats::stats(&conn, nikon).unwrap();
    assert_eq!(stats.photos, 1);
    let shutter = stats.shutter.clone().unwrap();
    assert_eq!((shutter.read, shutter.estimate), (Some(739), 739));
    assert_eq!(stats.focal.len(), 1);
    assert_eq!(stats.months.len(), 1);
    assert_eq!(stats.albums[0].name, "Matériel");
    assert!(stats.albums[0].favorite_lens.is_some(), "{stats:#?}");
    assert_eq!(stats.partners.len(), 1);
    let shutter_of = |model: &str| {
        let id = bodies.iter().find(|b| b.model.contains(model)).unwrap().id;
        gear::stats::stats(&conn, id).unwrap().shutter.unwrap()
    };
    assert_eq!(shutter_of("X-T3").read, Some(3850));
    let canon = shutter_of("R6");
    assert_eq!((canon.read, canon.estimate), (None, 1));
    let lens_stats = gear::stats::stats(&conn, stats.partners[0].id).unwrap();
    assert!(lens_stats.shutter.is_none());
    assert_eq!(lens_stats.partners[0].id, nikon);

    // Fusion : les photos et les noms EXIF du Sony passent au Nikon.
    // Le journal d'entretien suit la fiche fusionnée.
    let cleaning = MaintenanceInput {
        date: "2026-03-01".into(),
        kind: MaintenanceKind::Sensor,
        cost_cents: Some(4500),
        currency: Some("EUR".into()),
        notes: None,
        reminder_months: Some(6),
    };
    maintenance::add(&conn, sony, &cleaning).unwrap();
    let merged = gear::merge(&mut conn, sony, nikon).unwrap();
    let journal = maintenance::list(&conn, nikon).unwrap();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].input, cleaning);
    assert_eq!(merged.item.photo_count, 2);
    assert_eq!(merged.aliases.len(), 2, "{:?}", merged.aliases);
    assert!(matches!(
        gear::get(&conn, sony),
        Err(GearError::NotFound(_))
    ));
    let lens = items.iter().find(|g| g.kind == GearKind::Lens).unwrap().id;
    assert!(matches!(
        gear::merge(&mut conn, lens, nikon),
        Err(GearError::KindMismatch)
    ));
    // Un nouvel import du même Sony (mêmes noms EXIF) retrouve la fiche fusionnée.
    let (make, model, serial): (String, String, String) = conn
        .query_row(
            "SELECT exif_make, exif_model, serial FROM gear_aliases WHERE exif_model LIKE '%7M3%'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let again = library::find_or_create_gear(
        &conn,
        library::GearKind::Body,
        &make,
        &model,
        Some(serial.as_str()).filter(|s| !s.is_empty()),
    )
    .unwrap();
    assert_eq!(again, nikon);

    // Masquage : absent de l'inventaire, sauf sur demande.
    let ricoh = bodies.iter().find(|b| b.model.contains("GR")).unwrap().id;
    gear::set_hidden(&conn, ricoh, true).unwrap();
    assert!(
        !gear::list(&conn, false)
            .unwrap()
            .iter()
            .any(|g| g.id == ricoh)
    );
    assert!(
        gear::list(&conn, true)
            .unwrap()
            .iter()
            .any(|g| g.id == ricoh && g.hidden)
    );
    // Un appareil relevé dans les photos ne se supprime pas.
    let data_dir = dir.path().join("donnees");
    assert!(matches!(
        gear::delete(&conn, &data_dir, ricoh),
        Err(GearError::Detected)
    ));

    // Accessoire et fiche.
    let tripod = gear::create(&conn, GearKind::Tripod, "Manfrotto", "Befree").unwrap();
    let record = gear::update(
        &conn,
        tripod,
        GearUpdate {
            make: Some("Manfrotto".into()),
            model: Some("Befree Advanced".into()),
            name: Some("Trépied de voyage".into()),
            purchase_date: Some("2024-05-02".into()),
            price_cents: Some(18_990),
            currency: Some("EUR".into()),
            notes: Some("  ".into()),
            ..GearUpdate::default()
        },
    )
    .unwrap();
    assert_eq!(record.item.display_name, "Trépied de voyage");
    assert_eq!(record.item.model, "Befree Advanced");
    assert_eq!((record.price_cents, record.notes), (Some(18_990), None));
    assert!(record.item.confirmed && !record.item.auto_detected);

    // Photo déposée : copiée à côté de la base, la source intacte.
    let source = dir.path().join("trepied.jpg");
    fs::write(&source, b"\xFF\xD8 une image").unwrap();
    let before = common::sha256(&source);
    gear::set_photo_file(&conn, &data_dir, tripod, &source).unwrap();
    assert_eq!(
        gear::photo_file(&conn, &data_dir, tripod)
            .unwrap()
            .as_deref(),
        Some(&b"\xFF\xD8 une image"[..])
    );
    assert_eq!(common::sha256(&source), before);
    assert!(gear::set_photo_file(&conn, &data_dir, tripod, &dir.path().join("x.txt")).is_err());

    // Export : la photo déposée sert au PDF ; sans photo ni aperçu, rien.
    let cache = dir.path().join("cache");
    assert_eq!(
        gear::export::gear_photo(&conn, &data_dir, &cache, tripod).as_deref(),
        Some(&b"\xFF\xD8 une image"[..])
    );
    assert!(gear::export::gear_photo(&conn, &data_dir, &cache, nikon).is_none());

    // Retirer la photo efface la copie ; la source reste.
    gear::set_photo_media(&conn, &data_dir, tripod, None).unwrap();
    assert!(
        gear::photo_file(&conn, &data_dir, tripod)
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read_dir(data_dir.join("gear")).unwrap().count(), 0);
    assert_eq!(common::sha256(&source), before);

    gear::delete(&conn, &data_dir, tripod).unwrap();
    assert!(matches!(
        gear::get(&conn, tripod),
        Err(GearError::NotFound(_))
    ));
}
