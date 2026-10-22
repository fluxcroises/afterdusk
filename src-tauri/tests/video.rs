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

/// Dérushage : deux extraits exportés sans réencodage, chacun dans son
/// fichier puis réunis en un seul ; noms lisibles, original intact, rien
/// d'écrit à côté de lui.
#[test]
fn extraits_exportes_sans_reencodage() {
    if !video::available() {
        eprintln!("FFmpeg absent : test sauté (node scripts/fetch-ffmpeg.mjs)");
        return;
    }
    use afterdusk_lib::export::clips::export;
    use afterdusk_lib::library::clips::Clip;

    let dir = tempfile::tempdir().unwrap();
    let source_dir = dir.path().join("source");
    fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("C0002.MP4");
    fs::copy(fixture("rushes_gop12.mp4"), &source).unwrap();
    let before = common::sha256(&source);
    let clip = |id, start_ms, end_ms, name: &str| Clip {
        id,
        media_id: 1,
        start_ms,
        end_ms,
        name: name.into(),
    };
    // Un début hors image clé est recalé à l'export (960 ms).
    let clips = [
        clip(1, 1000, 2400, ""),
        clip(2, 2880, 3840, "Discours : fin"),
    ];
    let out = dir.path().join("Livraison");
    let mut steps = Vec::new();
    let files = export(&source, &clips, &out, false, |d, t| steps.push((d, t))).unwrap();
    assert_eq!(steps, [(1, 2), (2, 2)]);
    let names: Vec<_> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        ["C0002 - 00m00s-00m02s.mp4", "C0002 - Discours _ fin.mp4"]
    );
    let first = video::probe(&files[0]).unwrap();
    assert!(
        first.duration_ms >= 1400 && first.duration_ms <= 1700,
        "{}",
        first.duration_ms
    );

    let joined = export(&source, &clips, &out, true, |_, _| {}).unwrap();
    assert_eq!(joined.len(), 1);
    assert!(joined[0].ends_with("C0002 - extraits.mp4"));
    let total = video::probe(&joined[0]).unwrap().duration_ms;
    let parts: i64 = files
        .iter()
        .map(|f| video::probe(f).unwrap().duration_ms)
        .sum();
    assert!((total - parts).abs() <= 80, "{total} / {parts}");

    // Règles absolues : original intact, dossier source inchangé.
    assert_eq!(common::sha256(&source), before);
    assert_eq!(fs::read_dir(&source_dir).unwrap().count(), 1);
}
