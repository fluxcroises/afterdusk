//! Aperçus en cache à partir des vrais RAW de test-assets/.

mod common;

use std::time::Instant;

use afterdusk_lib::metadata::media::MediaFormat;
use afterdusk_lib::raw::preview::{self, SIZES};

const SAMPLES: &[(&str, u32)] = &[
    // (fichier, côté long de l'aperçu intégré)
    ("fujifilm_x-t3.RAF", 4416),
    ("canon_eos_r6.CR3", 5472),
    ("nikon_z6.NEF", 6048),
    ("sony_ilce-7m3.ARW", 1616),
    ("panasonic_dc-g9.RW2", 1920),
    ("ricoh_gr-iii.DNG", 6000),
];

#[test]
fn trois_paliers_pour_chaque_format() {
    let cache = tempfile::tempdir().unwrap();
    for (index, (file, embedded)) in SAMPLES.iter().enumerate() {
        let path = common::sample(file);
        let before = common::sha256(&path);
        let start = Instant::now();
        let out = preview::generate(&path, MediaFormat::Raw, 1, cache.path(), index as i64)
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        eprintln!("{file}: {} ms", start.elapsed().as_millis());

        for (generated, &size) in out.iter().zip(SIZES.iter()) {
            let long = generated.width.max(generated.height);
            assert_eq!(long, size.min(*embedded), "{file} palier {size}");
            let path = preview::cache_path(cache.path(), index as i64, size);
            assert!(std::fs::read(&path).unwrap().starts_with(&[0xff, 0xd8]));
        }
        // Règle absolue : générer un aperçu ne touche pas l'original.
        assert_eq!(common::sha256(&path), before, "{file}");
    }
}

#[test]
fn miniature_de_la_source() {
    let path = common::sample("nikon_z6.NEF");
    let jpeg = preview::thumbnail(&path, MediaFormat::Raw, 1, 256).unwrap();
    let img = image::load_from_memory(&jpeg).unwrap();
    assert_eq!(img.width().max(img.height()), 256);
}

/// Photo retouchée : aperçus rendus d'après la retouche (proportions du
/// recadrage), plus de JPEG intégré pour la visionneuse ; retouche annulée :
/// de nouveau l'aperçu du boîtier. L'original n'est jamais touché.
#[test]
fn apercus_rendus_d_apres_la_retouche() {
    use std::sync::atomic::AtomicBool;

    use afterdusk_lib::develop::settings::DevelopSettings;
    use afterdusk_lib::develop::store::{self, ORIGINAL};
    use afterdusk_lib::import::{self, ImportOptions};
    use afterdusk_lib::library::media::{full_size_jpeg, generate_previews, is_stale};
    use afterdusk_lib::{db, library};

    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    std::fs::create_dir_all(&card).unwrap();
    std::fs::copy(common::sample("nikon_z6.NEF"), card.join("z6.NEF")).unwrap();
    let root = dir.path().join("Bibliotheque");
    std::fs::create_dir_all(&root).unwrap();
    let cache = dir.path().join("cache");
    let mut conn = db::open(&dir.path().join("a.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let id = import::plan(
        &mut conn,
        &ImportOptions {
            library_id: lib.id,
            source_path: card.clone(),
            source_label: "Carte".into(),
            files: vec![card.join("z6.NEF")],
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
    let media: i64 = conn
        .query_row("SELECT id FROM media", [], |r| r.get(0))
        .unwrap();
    let original = root.join(
        conn.query_row("SELECT path FROM media", [], |r| r.get::<_, String>(0))
            .unwrap(),
    );
    let before = common::sha256(&original);
    let source = |conn: &rusqlite::Connection| -> String {
        conn.query_row(
            "SELECT source FROM previews WHERE media_id = ?1 AND size = 2048",
            [media],
            |r| r.get(0),
        )
        .unwrap()
    };

    generate_previews(&conn, &cache, media).unwrap();
    assert_eq!(source(&conn), "embedded");
    assert!(!is_stale(&conn, media));
    assert!(full_size_jpeg(&conn, media).unwrap().is_some());

    // Retouche : exposition et recadrage carré.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let mut s = DevelopSettings::default();
    s.light.exposure = 1.0;
    s.geometry.crop.left = 0.2;
    s.geometry.crop.right = 0.8;
    s.geometry.crop.bottom = 0.9;
    store::save(&mut conn, media, ORIGINAL, s, "geometry.crop").unwrap();
    assert!(is_stale(&conn, media));
    let out = generate_previews(&conn, &cache, media).unwrap();
    assert_eq!(source(&conn), "rendered");
    let big = &out[2];
    // 60 % de la largeur, 90 % de la hauteur d'une image 3:2.
    let ratio = f64::from(big.width) / f64::from(big.height);
    assert!((ratio - 1.0).abs() < 0.02, "{ratio}");
    assert_eq!(big.width.max(big.height), 2048);
    assert!(full_size_jpeg(&conn, media).unwrap().is_none());

    // Retouche remise à zéro : l'aperçu du boîtier revient.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    store::save(
        &mut conn,
        media,
        ORIGINAL,
        DevelopSettings::default(),
        "reset",
    )
    .unwrap();
    generate_previews(&conn, &cache, media).unwrap();
    assert_eq!(source(&conn), "embedded");
    assert_eq!(common::sha256(&original), before);
}
