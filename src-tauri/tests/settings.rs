//! Réglages de la première ouverture : identité, sauvegarde, modèles.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::db;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::identity::{self, Identity};
use afterdusk_lib::library::{self, paths, template};
use afterdusk_lib::metadata::xmp;

#[test]
fn l_identite_signe_les_photos_importees() {
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
    identity::set(
        &conn,
        Identity {
            name: "Lewis Bedar".into(),
            copyright: "© 2026 Lewis Bedar".into(),
            website: "https://example.org".into(),
        },
    )
    .unwrap();
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
        keywords: vec![],
        // L'interface ne précise pas l'auteur : celui de l'identité.
        creator: None,
        rights: Some("© 2026 Lewis Bedar".into()),
        skip_pair_jpegs: false,
    };
    let id = import::plan(&mut conn, &opts).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let sidecar: String = conn
        .query_row("SELECT sidecar_path FROM media", [], |r| r.get(0))
        .unwrap();
    let data = xmp::read_sidecar(&paths::to_absolute(&root, &sidecar).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(data.creator.as_deref(), Some("Lewis Bedar"));
    assert_eq!(data.rights.as_deref(), Some("© 2026 Lewis Bedar"));
}

#[test]
fn sauvegarde_et_modeles() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Bibliotheque");
    let backup = dir.path().join("Sauvegarde");
    fs::create_dir_all(&root).unwrap();
    let conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();

    // Un dossier de sauvegarde absent est refusé ; existant, il est retenu.
    assert!(library::set_backup(&conn, lib.id, Some(&backup)).is_err());
    fs::create_dir_all(&backup).unwrap();
    library::set_backup(&conn, lib.id, Some(&backup)).unwrap();
    assert_eq!(
        library::get_library(&conn, lib.id).unwrap().backup_path,
        Some(backup.clone())
    );
    library::set_backup(&conn, lib.id, None).unwrap();
    assert_eq!(
        library::get_library(&conn, lib.id).unwrap().backup_path,
        None
    );

    // Modèles : un jeton inconnu est refusé, l'aperçu montre le résultat.
    assert!(library::set_templates(&conn, lib.id, "{yyyy}/{lieu}", None).is_err());
    library::set_templates(
        &conn,
        lib.id,
        "{yyyy}/{mm}/{dd}",
        Some("{original}_{seq:3}"),
    )
    .unwrap();
    let lib = library::get_library(&conn, lib.id).unwrap();
    assert_eq!(lib.folder_template, "{yyyy}/{mm}/{dd}");
    assert_eq!(
        template::preview(
            &lib.folder_template,
            lib.rename_template.as_deref(),
            "Sortie"
        )
        .unwrap(),
        "2026/10/18/DSCF0042_042.RAF"
    );
    assert_eq!(
        template::preview(template::DEFAULT_FOLDER_TEMPLATE, None, "Sortie").unwrap(),
        "2026/2026-10-18 Sortie/DSCF0042.RAF"
    );
    assert_eq!(paths::same_volume(&root, &backup), Some(true));
}
