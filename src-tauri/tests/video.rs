//! Vidéos de bout en bout (décision 0012) : une carte rangée comme celle
//! d'un Sony (clip et XML dans PRIVATE/M4ROOT/CLIP), un MOV HEVC dans le
//! DCIM et une photo. Import avec annexe, informations, miniature, filtre,
//! corbeille aller-retour avec l'annexe, carte jamais écrite.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use afterdusk_lib::import::{self, ImportOptions, scan};
use afterdusk_lib::library::filter::MediaFilter;
use afterdusk_lib::library::media::{MediaQuery, generate_previews, list_media};
use afterdusk_lib::library::trash;
use afterdusk_lib::{db, library, video};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/video")
        .join(name)
}

fn hashes(dir: &Path) -> Vec<(PathBuf, String)> {
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
fn videos_importees_avec_leurs_annexes() {
    if !video::available() {
        eprintln!("FFmpeg absent : test sauté (node scripts/fetch-ffmpeg.mjs)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    let clip = card.join("PRIVATE/M4ROOT/CLIP");
    let dcim = card.join("DCIM/100MSDCF");
    fs::create_dir_all(&clip).unwrap();
    fs::create_dir_all(&dcim).unwrap();
    fs::copy(fixture("C0001.MP4"), clip.join("C0001.MP4")).unwrap();
    fs::copy(fixture("C0001M01.XML"), clip.join("C0001M01.XML")).unwrap();
    fs::copy(fixture("clip_hevc.mov"), dcim.join("C0002.MOV")).unwrap();
    fs::copy(
        common::sample(common::RAW_SAMPLES[0]),
        dcim.join("DSC0001.ARW"),
    )
    .unwrap();
    let card_before = hashes(&card);

    // L'analyse trouve les deux vidéos et la photo ; le XML n'est pas « ignoré ».
    let scanned = scan::scan(&card).unwrap();
    assert_eq!((scanned.files.len(), scanned.ignored), (3, 0));
    assert_eq!(
        scan::annexes(&clip.join("C0001.MP4")),
        vec![clip.join("C0001M01.XML")]
    );

    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("a.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let id = import::plan(
        &mut conn,
        &ImportOptions {
            library_id: lib.id,
            source_path: card.clone(),
            source_label: "Carte".into(),
            files: scanned.files.iter().map(|f| f.path.clone()).collect(),
            album_name: "Tournage".into(),
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

    let all = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    assert_eq!(all.len(), 3);
    let sony = all.iter().find(|m| m.file_name == "C0001.MP4").unwrap();
    assert_eq!(
        (sony.kind.as_str(), sony.duration_ms),
        ("video", Some(2000))
    );
    assert_eq!((sony.width, sony.height), (Some(1280), Some(720)));
    let hevc = all.iter().find(|m| m.file_name == "C0002.MOV").unwrap();
    let (fps, codec): (f64, String) = conn
        .query_row(
            "SELECT fps, codec FROM media_video WHERE media_id = ?1",
            [hevc.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((fps, codec.as_str()), (50.0, "hevc"));

    // L'annexe est copiée à côté de la vidéo, à l'identique.
    let (annex, original): (String, String) = conn
        .query_row(
            "SELECT path, original_name FROM media_annexes WHERE media_id = ?1",
            [sony.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(annex.ends_with("/C0001M01.XML"));
    assert_eq!(original, "C0001M01.XML");
    assert_eq!(
        common::sha256(&root.join(&annex)),
        common::sha256(&fixture("C0001M01.XML"))
    );

    // Filtre « vidéos » ; miniature calculée par ffmpeg.
    let videos = list_media(
        &conn,
        lib.id,
        &MediaQuery {
            filter: MediaFilter {
                kind: Some("video".into()),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(videos.len(), 2);
    let cache = dir.path().join("cache");
    let previews = generate_previews(&conn, &cache, hevc.id).unwrap();
    assert!(!previews.is_empty());

    // Corbeille : l'annexe part et revient avec sa vidéo.
    trash::move_to_trash(&mut conn, &[sony.id]).unwrap();
    let trashed: String = conn
        .query_row(
            "SELECT path FROM media_annexes WHERE media_id = ?1",
            [sony.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(trashed.starts_with(trash::TRASH_DIR) && root.join(&trashed).is_file());
    assert!(!root.join(&annex).exists());
    trash::restore(&mut conn, &[sony.id]).unwrap();
    assert!(root.join(&annex).is_file());

    // Règle absolue : la carte n'a pas été écrite.
    assert_eq!(card_before, hashes(&card));
}
