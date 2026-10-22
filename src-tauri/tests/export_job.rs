//! Export de bout en bout : import de vraies photos, puis export avec deux
//! presets (Web et Originaux) dans des sous-dossiers. Règles absolues :
//! aucun original modifié, copies d'originaux identiques.

mod common;

use std::fs;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::develop::settings::DevelopSettings;
use afterdusk_lib::develop::store;
use afterdusk_lib::export::design::{self, Align, Design, DesignPhoto, DesignText, Layout};
use afterdusk_lib::export::job::{self, ExportRequest, Target};
use afterdusk_lib::export::presets::{ExportSettings, Output};
use afterdusk_lib::export::templates;
use afterdusk_lib::export::watermark::{Anchor, Watermark, WatermarkContent};
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::{db, library};

#[test]
fn web_et_originaux_dans_des_sous_dossiers() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in ["nikon_z6.NEF", "sony_ilce-7m3.ARW"] {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    fs::write(card.join("note.JPG"), b"pas une vraie image").unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let db_path = dir.path().join("afterdusk.db");
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
            album_name: "Export".into(),
            album_per_day: false,
            rename: false,
            backup: false,
            keywords: vec!["client".into()],
            creator: Some("Lewis Bedar".into()),
            rights: Some("© 2026 Lewis Bedar".into()),
            skip_pair_jpegs: false,
            client_id: None,
            session_type_id: None,
            session_label: String::new(),
        },
    )
    .unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM media ORDER BY path")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(ids.len(), 3);
    // Une retouche sur la première photo.
    let mut s = DevelopSettings::default();
    s.light.exposure = 0.5;
    store::save(&mut conn, ids[0], store::ORIGINAL, s, "light.exposure").unwrap();

    let originals: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .flat_map(|e| walk(&e.unwrap().path()))
        .filter(|p| p.extension().is_some_and(|e| e != "xmp"))
        .map(|p| (p.clone(), common::sha256(&p)))
        .collect();

    let out = dir.path().join("Livraison");
    let mut settings = ExportSettings {
        watermark: Some(Watermark {
            content: WatermarkContent::Text {
                text: "© Lewis Bedar".into(),
                dark: false,
            },
            anchor: Anchor::BottomRight,
            size: 20.0,
            opacity: 60.0,
            margin: 3.0,
        }),
        ..ExportSettings::default()
    };
    settings
        .presets
        .iter_mut()
        .find(|p| p.id == "web")
        .unwrap()
        .watermark = true;
    let request = ExportRequest {
        media_ids: ids.clone(),
        targets: vec![
            Target {
                preset: "web".into(),
                folder: Some("Web".into()),
                template: None,
            },
            Target {
                preset: "originals".into(),
                folder: Some("Originaux".into()),
                template: None,
            },
        ],
        destination: out.clone(),
    };
    let reports = Mutex::new(Vec::new());
    let done = job::run(
        &db_path,
        &settings,
        &request,
        &AtomicBool::new(false),
        &|p| {
            reports.lock().unwrap().push(p.done);
        },
    )
    .unwrap();
    assert_eq!(done.total, 6);
    assert_eq!(done.done, 6);
    assert_eq!(reports.lock().unwrap().len(), 3);
    // Le faux JPEG ne se rend pas (Web) ; il se copie (Originaux).
    assert_eq!(done.failures.len(), 1, "{:?}", done.failures);
    assert_eq!(done.failures[0].preset, "web");

    let web: Vec<_> = walk(&out.join("Web"));
    assert_eq!(web.len(), 2, "{web:?}");
    for p in &web {
        let img = image::open(p).unwrap();
        assert_eq!(img.width().max(img.height()), 2048);
    }
    // Originaux : copies identiques, XMP compris.
    let copies = walk(&out.join("Originaux"));
    assert_eq!(
        copies
            .iter()
            .filter(|p| p.extension().unwrap() == "xmp")
            .count(),
        3
    );
    for (orig, hash) in &originals {
        let copy = out.join("Originaux").join(orig.file_name().unwrap());
        assert_eq!(&common::sha256(&copy), hash);
        // Règle absolue : l'original n'a pas bougé.
        assert_eq!(&common::sha256(orig), hash);
    }
    // Deuxième export : rien n'est écrasé.
    job::run(
        &db_path,
        &settings,
        &request,
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    assert_eq!(walk(&out.join("Web")).len(), 4);
}

#[test]
fn heif_avec_copyright_quand_le_systeme_sait_l_ecrire() {
    if !afterdusk_lib::export::heif::available() {
        eprintln!("pas d'encodeur HEIF sur ce système : test sauté");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    fs::copy(common::sample("nikon_z6.NEF"), card.join("nikon_z6.NEF")).unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let db_path = dir.path().join("afterdusk.db");
    let mut conn = db::open(&db_path).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let id = import::plan(
        &mut conn,
        &ImportOptions {
            library_id: lib.id,
            source_path: card.clone(),
            source_label: "Carte".into(),
            files: vec![card.join("nikon_z6.NEF")],
            album_name: "HEIF".into(),
            album_per_day: false,
            rename: false,
            backup: false,
            keywords: vec![],
            creator: Some("Lewis Bedar".into()),
            rights: Some("© 2026 Lewis Bedar".into()),
            skip_pair_jpegs: false,
            client_id: None,
            session_type_id: None,
            session_label: String::new(),
        },
    )
    .unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let media: i64 = conn
        .query_row("SELECT id FROM media", [], |r| r.get(0))
        .unwrap();
    let original = walk(&root)
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "NEF"))
        .unwrap();
    let hash = common::sha256(&original);

    let mut settings = ExportSettings::default();
    settings
        .presets
        .iter_mut()
        .find(|p| p.id == "web")
        .unwrap()
        .output = Output::Heif { quality: 80 };
    let out = dir.path().join("Livraison");
    let done = job::run(
        &db_path,
        &settings,
        &ExportRequest {
            media_ids: vec![media],
            targets: vec![Target {
                preset: "web".into(),
                folder: None,
                template: None,
            }],
            destination: out.clone(),
        },
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    assert!(done.failures.is_empty(), "{:?}", done.failures);
    let files = walk(&out);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0].extension().unwrap(), "heic");
    let bytes = fs::read(&files[0]).unwrap();
    assert_eq!(&bytes[4..8], b"ftyp");
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    assert!(has("© 2026 Lewis Bedar".as_bytes()), "copyright absent");
    // Bloc EXIF (en-tête TIFF) avec la marque du boîtier, quelle que soit sa casse.
    assert!(has(b"II*\0") || has(b"MM\0*"), "EXIF absent");
    assert!(
        bytes.to_ascii_lowercase().windows(5).any(|w| w == b"nikon"),
        "marque absente"
    );
    // Règle absolue : l'original n'a pas bougé.
    assert_eq!(common::sha256(&original), hash);
}

/// Les JPEG, TIFF et PNG se rendent comme les RAW : taille du preset,
/// orientation appliquée, original intact.
#[test]
fn jpeg_et_png_rendus_par_le_preset_web() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    let img = image::RgbImage::from_fn(3000, 2000, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    img.save(card.join("paysage.jpg")).unwrap();
    image::DynamicImage::ImageRgb8(img)
        .into_rgb16()
        .save(card.join("paysage16.png"))
        .unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let db_path = dir.path().join("afterdusk.db");
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
            album_name: "Images".into(),
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
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM media ORDER BY path")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(ids.len(), 2);
    let originals: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .flat_map(|e| walk(&e.unwrap().path()))
        .filter(|p| p.extension().is_some_and(|e| e != "xmp"))
        .map(|p| (p.clone(), common::sha256(&p)))
        .collect();
    assert_eq!(originals.len(), 2);

    let out = dir.path().join("Livraison");
    let request = ExportRequest {
        media_ids: ids,
        targets: vec![Target {
            preset: "web".into(),
            folder: None,
            template: None,
        }],
        destination: out.clone(),
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
    let web = walk(&out);
    assert_eq!(web.len(), 2, "{web:?}");
    for p in &web {
        let img = image::open(p).unwrap();
        assert_eq!((img.width(), img.height()), (2048, 1365), "{p:?}");
    }
    for (orig, hash) in &originals {
        assert_eq!(&common::sha256(orig), hash);
    }
}

/// Modèles de livraison : taille du modèle, tirage tourné avec la photo,
/// cadrage enregistré respecté.
#[test]
fn modeles_de_livraison() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    // Moitié gauche rouge, moitié droite bleue.
    image::RgbImage::from_fn(3000, 2000, |x, _| {
        if x < 1500 {
            image::Rgb([220, 20, 20])
        } else {
            image::Rgb([20, 20, 220])
        }
    })
    .save(card.join("paysage.jpg"))
    .unwrap();
    image::RgbImage::from_pixel(2000, 3000, image::Rgb([90, 90, 90]))
        .save(card.join("portrait.jpg"))
        .unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let db_path = dir.path().join("afterdusk.db");
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
            album_name: "Modèles".into(),
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
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM media ORDER BY path")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(ids.len(), 2);
    let landscape = ids[0];
    // Cadre 4:5 enregistré dans la moitié rouge.
    templates::set(
        &conn,
        landscape,
        "instagram-portrait",
        [0.0, 0.0, 0.4, 0.75],
    )
    .unwrap();

    let out = dir.path().join("Livraison");
    let target = |template: &str| Target {
        preset: "web".into(),
        folder: Some(template.into()),
        template: Some(template.into()),
    };
    let request = ExportRequest {
        media_ids: ids,
        targets: vec![target("instagram-portrait"), target("print-10x15")],
        destination: out.clone(),
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
    let size = |p: &std::path::Path| {
        let img = image::open(p).unwrap();
        (img.width(), img.height())
    };
    let insta = out.join("instagram-portrait");
    let print = out.join("print-10x15");
    assert_eq!(size(&insta.join("paysage.jpg")), (1080, 1350));
    assert_eq!(size(&insta.join("portrait.jpg")), (1080, 1350));
    assert_eq!(size(&print.join("paysage.jpg")), (1800, 1200));
    assert_eq!(size(&print.join("portrait.jpg")), (1200, 1800));
    // Le cadre enregistré ne prend que du rouge.
    let red = image::open(insta.join("paysage.jpg")).unwrap().into_rgb8();
    let p = red.get_pixel(1079, 675);
    assert!(p[0] > 150 && p[2] < 80, "{p:?}");
    // Par défaut, centré : du rouge à gauche, du bleu à droite.
    let both = image::open(print.join("paysage.jpg")).unwrap().into_rgb8();
    assert!(both.get_pixel(10, 600)[0] > 150);
    assert!(both.get_pixel(1790, 600)[2] > 150);
}

/// Éditeur de livraison : deux photos côte à côte sur un fond, un texte,
/// exportées à la taille du modèle ; originaux intacts.
#[test]
fn composition_de_deux_photos() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    image::RgbImage::from_pixel(3000, 2000, image::Rgb([220, 20, 20]))
        .save(card.join("a_rouge.jpg"))
        .unwrap();
    image::RgbImage::from_pixel(2000, 3000, image::Rgb([20, 20, 220]))
        .save(card.join("b_bleu.jpg"))
        .unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let db_path = dir.path().join("afterdusk.db");
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
            album_name: "Composition".into(),
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
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM media ORDER BY path")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let originals: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .flat_map(|e| walk(&e.unwrap().path()))
        .filter(|p| p.extension().is_some_and(|e| e != "xmp"))
        .map(|p| (p.clone(), common::sha256(&p)))
        .collect();

    let doc = Design {
        version: design::VERSION,
        template: "facebook-post".into(),
        layout: Layout::Grid,
        width: 1200,
        height: 630,
        photos: ids
            .iter()
            .map(|&item| DesignPhoto {
                item,
                focus: [0.5, 0.5],
            })
            .collect(),
        background: "#ffffff".into(),
        margin: 5.0,
        frame: None,
        texts: vec![DesignText {
            text: "Léa & Hugo".into(),
            x: 0.5,
            y: 0.45,
            size: 8.0,
            font: "Playfair Display".into(),
            weight: 700,
            color: "#000000".into(),
            align: Align::Center,
        }],
        logo: None,
    };
    let saved = design::save(&conn, None, "Faire-part : juin", &doc).unwrap();
    let doc = design::get(&conn, saved).unwrap().unwrap().document;
    let out = dir.path().join("Livraison");
    let file = job::export_design(
        &conn,
        &ExportSettings::default(),
        &doc,
        &out,
        "Faire-part : juin",
    )
    .unwrap();
    let img = image::open(&file).unwrap().into_rgb8();
    assert_eq!((img.width(), img.height()), (1200, 630));
    // Fond blanc dans la marge, rouge à gauche, bleu à droite.
    let px = |x: u32, y: u32| img.get_pixel(x, y).0;
    assert!(px(5, 5).iter().all(|&c| c > 240), "{:?}", px(5, 5));
    assert!(px(200, 100)[0] > 180 && px(200, 100)[2] < 70);
    assert!(px(1000, 100)[2] > 180 && px(1000, 100)[0] < 70);

    // Bande photomaton doublée (10 × 15) : deux colonnes identiques, le
    // rouge puis le bleu, bandeau noir en bas.
    let strip = Design {
        template: "photo-strip".into(),
        layout: Layout::Strip,
        width: 1200,
        height: 1800,
        background: "#000000".into(),
        margin: 4.0,
        texts: Vec::new(),
        ..doc.clone()
    };
    let file =
        job::export_design(&conn, &ExportSettings::default(), &strip, &out, "Bande").unwrap();
    let img = image::open(&file).unwrap().into_rgb8();
    assert_eq!((img.width(), img.height()), (1200, 1800));
    let px = |x: u32, y: u32| img.get_pixel(x, y).0;
    for x in [300, 900] {
        assert!(
            px(x, 400)[0] > 180 && px(x, 400)[2] < 70,
            "{x} : {:?}",
            px(x, 400)
        );
        assert!(
            px(x, 1100)[2] > 180 && px(x, 1100)[0] < 70,
            "{x} : {:?}",
            px(x, 1100)
        );
        assert!(px(x, 1750).iter().all(|&c| c < 20));
    }
    for (orig, hash) in &originals {
        assert_eq!(&common::sha256(orig), hash);
    }
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    if dir.is_file() {
        return vec![dir.to_path_buf()];
    }
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            out.extend(walk(&e.path()));
        }
    }
    out.sort();
    out
}
