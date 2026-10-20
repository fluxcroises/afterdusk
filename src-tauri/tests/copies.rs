//! Copies virtuelles dans les grilles : listées après leur photo, aperçu
//! rendu à part, marques partagées, exportées avec leurs réglages.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::develop::settings::DevelopSettings;
use afterdusk_lib::develop::store::{self, ORIGINAL};
use afterdusk_lib::export::job::{self, ExportRequest, Target};
use afterdusk_lib::export::presets::ExportSettings;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::copies;
use afterdusk_lib::library::media::{MediaQuery, generate_previews, list_media};
use afterdusk_lib::{db, library};

#[test]
fn copie_listee_rendue_et_exportee() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in ["nikon_z6.NEF", "sony_ilce-7m3.ARW"] {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let cache = dir.path().join("cache");
    let db_path = dir.path().join("a.db");
    let mut conn = db::open(&db_path).unwrap();
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
            album_name: "A".into(),
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
    let query = MediaQuery::default();
    let before = list_media(&conn, lib.id, &query).unwrap();
    assert_eq!(before.len(), 2);
    let photo = before[0].id;

    // Une copie en noir et blanc.
    store::create_version(&mut conn, photo, ORIGINAL, "Noir et blanc").unwrap();
    let mut bw = DevelopSettings::default();
    bw.color.saturation = -100.0;
    store::save(&mut conn, photo, "Noir et blanc", bw, "color.saturation").unwrap();

    let list = list_media(&conn, lib.id, &query).unwrap();
    assert_eq!(list.len(), 3);
    let copy = &list[1];
    assert!(copy.id < 0);
    assert_eq!(copy.copy_of, Some(photo));
    assert_eq!(copy.version.as_deref(), Some("Noir et blanc"));
    assert_eq!(copy.file_name, list[0].file_name);
    // Les marques d'une copie sont celles de sa photo.
    assert_eq!(
        copies::media_ids(&conn, &[copy.id, photo]).unwrap(),
        vec![photo]
    );
    // La corbeille ne liste pas les copies.
    let trash = MediaQuery {
        trashed: true,
        ..MediaQuery::default()
    };
    assert!(list_media(&conn, lib.id, &trash).unwrap().is_empty());

    // Aperçu rendu à part, en gris ; ensuite à jour.
    assert_eq!(
        copies::stale_copies(&conn, &cache, None).unwrap(),
        vec![copy.id]
    );
    // Dates à la seconde : l'aperçu doit être d'une seconde plus récente.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let out = generate_previews(&conn, &cache, copy.id).unwrap();
    assert_eq!(out[2].width.max(out[2].height), 2048);
    assert!(copies::has_previews(&cache, copy.id));
    assert!(
        copies::stale_copies(&conn, &cache, None)
            .unwrap()
            .is_empty()
    );
    let img = image::open(afterdusk_lib::raw::preview::cache_path(
        &cache, copy.id, 256,
    ))
    .unwrap()
    .to_rgb8();
    assert!(
        img.pixels()
            .all(|p| p[0].abs_diff(p[1]) <= 3 && p[1].abs_diff(p[2]) <= 3)
    );

    // Export : la photo et sa copie, la copie nommée d'après sa version ;
    // le preset des originaux ignore la copie (même fichier).
    let out_dir = dir.path().join("Livraison");
    let request = ExportRequest {
        media_ids: vec![photo, copy.id],
        targets: vec![
            Target {
                preset: "web".into(),
                folder: Some("Web".into()),
            },
            Target {
                preset: "originals".into(),
                folder: Some("Originaux".into()),
            },
        ],
        destination: out_dir.clone(),
    };
    let done = job::run(
        &db_path,
        &ExportSettings::default(),
        &request,
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    assert!(done.failures.is_empty(), "{:?}", done.failures);
    let mut web: Vec<String> = fs::read_dir(out_dir.join("Web"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    web.sort();
    assert_eq!(web.len(), 2, "{web:?}");
    assert!(
        web.iter().any(|n| n.ends_with(" - Noir et blanc.jpg")),
        "{web:?}"
    );
    let originals = fs::read_dir(out_dir.join("Originaux"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x != "xmp")
        })
        .count();
    assert_eq!(originals, 1);

    // Aperçu du résultat dans Livrer : taille du fichier, image réduite.
    let settings = ExportSettings::default();
    let web = settings.presets.iter().find(|p| p.id == "web").unwrap();
    let ((w, h), jpeg) = job::preview(&conn, copy.id, web, None, 900).unwrap();
    assert_eq!(w.max(h), 2048);
    let img = image::load_from_memory(&jpeg.unwrap()).unwrap();
    assert_eq!(img.width().max(img.height()), 900);
    let originals = settings
        .presets
        .iter()
        .find(|p| p.id == "originals")
        .unwrap();
    assert_eq!(
        job::preview(&conn, photo, originals, None, 900).unwrap(),
        ((0, 0), None)
    );
}
