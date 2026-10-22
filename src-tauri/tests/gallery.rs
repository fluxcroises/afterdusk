//! Galerie web de bout en bout : deux RAW importés, galerie générée dans un
//! dossier, fichiers sans métadonnées, originaux intacts, puis sélection du
//! client relue jusqu'aux photos de la bibliothèque.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::analysis::run::source_image;
use afterdusk_lib::export::contact_sheet::SheetWatermark;
use afterdusk_lib::export::gallery::{self, GalleryRequest, Theme};
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::media::{MediaQuery, list_media};
use afterdusk_lib::{db, library};

/// Empreintes de tous les fichiers d'un dossier, dans l'ordre des chemins.
fn hashes(dir: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(hashes(&path));
        } else {
            out.push((path.clone(), common::sha256(&path)));
        }
    }
    out.sort();
    out
}

#[test]
fn galerie_generee_puis_selection_relue() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    let names = &common::RAW_SAMPLES[..2];
    for name in names {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
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
            album_name: "Essai".into(),
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
    let media = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    let stored = hashes(&root);

    let cache = dir.path().join("cache");
    let out = dir.path().join("Galeries");
    fs::create_dir_all(&out).unwrap();
    let request = GalleryRequest {
        library_id: lib.id,
        title: "Mariage Dupont".into(),
        subtitle: "Famille Dupont".into(),
        intro: String::new(),
        theme: Theme::Light,
        allow_download: false,
        selection: true,
        watermark: SheetWatermark::Text("© Studio".into()),
        album_id: None,
        client_id: None,
        // La dernière n'existe pas : elle est comptée comme manquante.
        media_ids: media.iter().map(|m| m.id).chain([999_999]).collect(),
        destination: out.clone(),
        texts: Default::default(),
    };
    let mut seen = 0;
    let result = gallery::generate(
        &conn,
        &request,
        "studio@example.com",
        |id| {
            source_image(&conn, &cache, id, gallery::PHOTO_SIDE).map(image::DynamicImage::ImageRgb8)
        },
        |done, _| seen = done,
    )
    .unwrap();
    assert_eq!((result.photos, result.missing, seen), (2, 1, 3));
    assert_eq!(result.folder, out.join("Mariage Dupont"));
    for file in [
        "index.html",
        "galerie.css",
        "galerie.js",
        "photos/0002.jpg",
        "vignettes/0002.jpg",
    ] {
        assert!(result.folder.join(file).is_file(), "{file}");
    }
    // Aucune métadonnée : ni EXIF (donc ni GPS), ni XMP.
    let photo = fs::read(result.folder.join("photos/0001.jpg")).unwrap();
    assert!(!photo.windows(4).any(|w| w == b"Exif"));
    assert!(!photo.windows(9).any(|w| w == b"x:xmpmeta"));
    let side = image::load_from_memory(&photo).unwrap();
    assert!(side.width().max(side.height()) <= gallery::PHOTO_SIDE);
    // Une deuxième galerie du même nom ne remplace pas la première.
    let again = gallery::generate(&conn, &request, "", |_| None, |_, _| {}).unwrap();
    assert_eq!(again.folder, out.join("Mariage Dupont (2)"));
    assert!(result.folder.join("photos/0001.jpg").is_file());

    // Le client renvoie la photo 2 (et un numéro qui n'existe pas).
    let mail = format!("Ma sélection\n[{}: 2 7]", result.code);
    let parsed = gallery::parse_selection(&mail).unwrap();
    let found = gallery::match_selection(&conn, &parsed).unwrap().unwrap();
    assert_eq!(found.title, "Mariage Dupont");
    assert_eq!(found.media_ids, vec![media[1].id]);
    assert_eq!(found.unknown, vec![7]);
    let other = gallery::parse_selection("[AD-ABCDEFGH: 1]").unwrap();
    assert!(gallery::match_selection(&conn, &other).unwrap().is_none());

    // Règle absolue : les fichiers de la bibliothèque n'ont pas changé.
    let after = hashes(&root);
    assert_eq!(stored, after);
}

/// Galerie d'exemple à regarder :
/// `AFTERDUSK_SAMPLE_GALLERY=dossier cargo test --test gallery -- --ignored`.
#[test]
#[ignore]
fn galerie_exemple() {
    let target = std::path::PathBuf::from(std::env::var("AFTERDUSK_SAMPLE_GALLERY").unwrap());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let fr: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("src/i18n/fr.json")).unwrap()).unwrap();
    let mut texts: std::collections::BTreeMap<String, String> = fr["gallery"]["page"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
        .collect();
    texts.insert("lang".into(), "fr".into());
    let faces = [
        image::open(root.join("test-assets/visage_ouverts.jpg")).unwrap(),
        image::open(root.join("test-assets/visage_fermes.jpg")).unwrap(),
    ];
    let conn = db::open_in_memory().unwrap();
    let lib_dir = target.join("bibliotheque");
    fs::create_dir_all(&lib_dir).unwrap();
    let lib = library::create_library(&conn, "Photos", &lib_dir, None).unwrap();
    let request = GalleryRequest {
        library_id: lib.id,
        title: "Mariage Dupont".into(),
        subtitle: "Famille Dupont · Mariage · 13 juin 2026 · Lewis Bedar".into(),
        intro: "Merci pour cette belle journée !\nChoisissez vos photos préférées pour l'album."
            .into(),
        theme: Theme::Light,
        allow_download: true,
        selection: true,
        watermark: SheetWatermark::Text("© Lewis Bedar".into()),
        album_id: None,
        client_id: None,
        media_ids: (1..=9).collect(),
        destination: target.clone(),
        texts,
    };
    let result = gallery::generate(
        &conn,
        &request,
        "studio@example.com",
        |id| Some(faces[(id % 2) as usize].clone()),
        |_, _| {},
    )
    .unwrap();
    println!("{}", result.folder.display());
}
