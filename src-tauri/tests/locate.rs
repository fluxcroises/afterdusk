//! Localisation de bout en bout : vrais RAW importés, puis trace GPX et
//! photo de téléphone. Règles absolues : aucun original modifié, aucune
//! écriture sur les fichiers GPX ni sur la photo de téléphone.

mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use chrono::{Duration, NaiveDateTime};
use rusqlite::Connection;

use afterdusk_lib::export::metadata::{MetadataPolicy, PhotoMeta, embedded};
use afterdusk_lib::geo::locate::{LocateOptions, locate_gpx, locate_phone};
use afterdusk_lib::geo::matching::Clock;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::marks::flush_pending_xmp;
use afterdusk_lib::metadata::media::MediaMetadata;
use afterdusk_lib::metadata::xmp::{LocationSource, read_sidecar};
use afterdusk_lib::raw::Gps;
use afterdusk_lib::{db, library};

struct Photo {
    id: i64,
    taken: NaiveDateTime,
}

/// Importe les RAW et rend leur identifiant et leur heure de prise de vue.
fn setup(dir: &Path, names: &[&str]) -> (Connection, std::path::PathBuf, Vec<Photo>) {
    let card = dir.join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in names {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let files = names.iter().map(|n| card.join(n)).collect();
    let id = import::plan(
        &mut conn,
        &ImportOptions {
            library_id: lib.id,
            source_path: card.clone(),
            source_label: "Carte".into(),
            files,
            album_name: "Lieux".into(),
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
    let photos = conn
        .prepare("SELECT id, captured_at FROM media ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
        .map(|r| {
            let (id, at) = r.unwrap();
            Photo {
                id,
                taken: NaiveDateTime::parse_from_str(&at[..19], "%Y-%m-%dT%H:%M:%S").unwrap(),
            }
        })
        .collect();
    (conn, root, photos)
}

fn located(conn: &Connection, id: i64) -> Option<(f64, f64, String)> {
    conn.query_row(
        "SELECT lat, lon, location_source FROM media WHERE id = ?1 AND lat IS NOT NULL",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .ok()
}

fn hashes(root: &Path) -> Vec<(std::path::PathBuf, String)> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|x| x != "xmp"))
        .map(|e| (e.path().to_path_buf(), common::sha256(e.path())))
        .collect()
}

#[test]
fn trace_gpx_puis_xmp_sans_toucher_aux_originaux() {
    let dir = tempfile::tempdir().unwrap();
    let (mut conn, root, photos) = setup(dir.path(), &["nikon_z6.NEF", "sony_ilce-7m3.ARW"]);
    let before = hashes(&root);
    // Boîtier réglé en UTC : la trace passe par chaque photo, à 30 s près.
    let mut points = String::new();
    for (i, p) in photos.iter().enumerate() {
        for (k, dt) in [(0.0, -30), (0.01, 30)] {
            let t = p.taken + Duration::seconds(dt);
            points += &format!(
                "<trkpt lat=\"{}\" lon=\"{}\"><time>{}Z</time></trkpt>\n",
                49.0 + i as f64 + k,
                -0.1 - k,
                t.format("%Y-%m-%dT%H:%M:%S")
            );
        }
    }
    let gpx = dir.path().join("montre.gpx");
    fs::write(
        &gpx,
        format!("<gpx version=\"1.1\"><trk><trkseg>{points}</trkseg></trk></gpx>"),
    )
    .unwrap();
    let gpx_hash = common::sha256(&gpx);
    let options = LocateOptions {
        clock: Clock::Offset(Duration::zero()),
        replace: false,
    };
    let ids: Vec<i64> = photos.iter().map(|p| p.id).collect();
    let report = locate_gpx(
        &mut conn,
        &ids,
        std::slice::from_ref(&gpx),
        options,
        Duration::minutes(10),
    )
    .unwrap();
    assert_eq!(report.located, 2, "{report:?}");
    for (i, p) in photos.iter().enumerate() {
        let (lat, lon, source) = located(&conn, p.id).unwrap();
        // Au milieu des deux points qui l'encadrent.
        assert!((lat - (49.005 + i as f64)).abs() < 1e-6, "{lat}");
        assert!((lon + 0.105).abs() < 1e-6, "{lon}");
        assert_eq!(source, "gpx");
    }
    let tracks: i64 = conn
        .query_row("SELECT count(*) FROM gpx_tracks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tracks, 1);

    // Une seconde fois : les lieux posés sont gardés.
    let again = locate_gpx(
        &mut conn,
        &ids,
        std::slice::from_ref(&gpx),
        options,
        Duration::minutes(10),
    )
    .unwrap();
    assert_eq!((again.located, again.kept), (0, 2));

    // Le lieu part dans le XMP, rien d'autre ne bouge.
    flush_pending_xmp(&conn, usize::MAX).unwrap();
    let sidecar: String = conn
        .query_row(
            "SELECT sidecar_path FROM media WHERE id = ?1",
            [photos[0].id],
            |r| r.get(0),
        )
        .unwrap();
    let loc = read_sidecar(&root.join(sidecar))
        .unwrap()
        .unwrap()
        .location
        .unwrap();
    assert!((loc.latitude - 49.005).abs() < 1e-6);
    assert_eq!(loc.source, LocationSource::Gpx);
    assert_eq!(hashes(&root), before);
    assert_eq!(common::sha256(&gpx), gpx_hash);

    // Base perdue : la réanalyse retrouve le lieu dans le XMP.
    let mut fresh = db::open(&dir.path().join("nouvelle.db")).unwrap();
    let lib = library::create_library(&fresh, "Photos", &root, None).unwrap();
    import::rescan::rescan(&mut fresh, lib.id, &AtomicBool::new(false), |_, _| {}).unwrap();
    let restored: Vec<(f64, String)> = fresh
        .prepare("SELECT lat, location_source FROM media WHERE lat IS NOT NULL ORDER BY lat")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(restored.len(), 2, "{restored:?}");
    assert!((restored[0].0 - 49.005).abs() < 1e-6);
    assert_eq!(restored[0].1, "gpx");
    assert_eq!(hashes(&root), before);
}

/// Photo de téléphone : JPEG avec heure, fuseau et position.
fn phone_photo(path: &Path, taken: NaiveDateTime, offset: &str, lat: f64, lon: f64) {
    let meta = PhotoMeta {
        camera: MediaMetadata {
            make: Some("Apple".into()),
            captured_at: Some(taken.format("%Y-%m-%dT%H:%M:%S").to_string()),
            utc_offset: Some(offset.into()),
            gps: Some(Gps {
                latitude: lat,
                longitude: lon,
                altitude: None,
            }),
            ..MediaMetadata::default()
        },
        sidecar: None,
    };
    let exif = embedded(&meta, MetadataPolicy::All).unwrap().exif.unwrap();
    let mut out = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, 80);
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&exif);
    encoder.add_app_segment(1, app1).unwrap();
    encoder
        .encode(&[128u8; 8 * 8 * 3], 8, 8, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    fs::write(path, out).unwrap();
}

#[test]
fn photo_de_telephone_du_meme_moment() {
    let dir = tempfile::tempdir().unwrap();
    let (mut conn, root, photos) = setup(dir.path(), &["nikon_z6.NEF"]);
    let before = hashes(&root);
    let shot = &photos[0];
    // Le téléphone, à l'heure de Paris (+02:00), 10 minutes après la photo
    // d'un boîtier réglé lui aussi à l'heure de Paris.
    let phone = dir.path().join("IMG_0042.JPG");
    phone_photo(
        &phone,
        shot.taken + Duration::minutes(10),
        "+02:00",
        43.2965,
        5.3698,
    );
    let useless = dir.path().join("IMG_0043.JPG");
    fs::write(&useless, b"pas une photo").unwrap();
    let phone_hash = common::sha256(&phone);
    let options = LocateOptions {
        clock: Clock::Offset(Duration::hours(2)),
        replace: false,
    };
    let files = [phone.clone(), useless];
    // Fenêtre de 5 minutes : trop courte.
    let r = locate_phone(&mut conn, &[shot.id], &files, options, Duration::minutes(5)).unwrap();
    assert_eq!((r.located, r.unmatched, r.unusable_sources), (0, 1, 1));
    let r = locate_phone(
        &mut conn,
        &[shot.id],
        &files,
        options,
        Duration::minutes(30),
    )
    .unwrap();
    assert_eq!(r.located, 1, "{r:?}");
    let (lat, lon, source) = located(&conn, shot.id).unwrap();
    assert!((lat - 43.2965).abs() < 1e-4 && (lon - 5.3698).abs() < 1e-4);
    assert_eq!(source, "phone");
    assert_eq!(hashes(&root), before);
    assert_eq!(common::sha256(&phone), phone_hash);
}
