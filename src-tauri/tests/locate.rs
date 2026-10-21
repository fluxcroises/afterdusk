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
use afterdusk_lib::geo::locate::{LocateOptions, locate_gpx, locate_manual, locate_phone};
use afterdusk_lib::geo::matching::Clock;
use afterdusk_lib::geo::naming::name_pending;
use afterdusk_lib::geo::time;
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

fn place(conn: &Connection, id: i64) -> (Option<String>, Option<String>, Option<String>) {
    conn.query_row(
        "SELECT city, country, timezone FROM media WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

fn found(conn: &Connection, words: &str) -> Vec<i64> {
    conn.prepare("SELECT rowid FROM media_search WHERE media_search MATCH ?1")
        .unwrap()
        .query_map([words], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn lieu_nomme_recherche_et_saisie_manuelle() {
    let dir = tempfile::tempdir().unwrap();
    let (mut conn, root, photos) = setup(dir.path(), &["nikon_z6.NEF"]);
    let before = hashes(&root);
    let id = photos[0].id;
    // Saisie : Cabourg.
    let r = locate_manual(&mut conn, &[id], 49.2869, -0.1166, false).unwrap();
    assert_eq!(r.located, 1);
    assert_eq!(located(&conn, id).unwrap().2, "manual");
    assert_eq!(
        place(&conn, id),
        (
            Some("Cabourg".into()),
            Some("FR".into()),
            Some("Europe/Paris".into())
        )
    );
    // Recherche par ville, par pays, sans accents ni casse.
    assert_eq!(found(&conn, "cabourg"), vec![id]);
    assert_eq!(found(&conn, "france"), vec![id]);
    // Lieu déjà posé : gardé, sauf demande.
    let r = locate_manual(&mut conn, &[id], 43.2965, 5.3698, false).unwrap();
    assert_eq!((r.located, r.kept), (0, 1));
    locate_manual(&mut conn, &[id], 43.2965, 5.3698, true).unwrap();
    assert_eq!(place(&conn, id).0.as_deref(), Some("Marseille"));
    assert!(found(&conn, "cabourg").is_empty());
    assert_eq!(found(&conn, "marseille"), vec![id]);

    // Base antérieure : lieu sans nom, nommé au démarrage.
    conn.execute(
        "UPDATE media SET city = NULL, country = NULL WHERE id = ?1",
        [id],
    )
    .unwrap();
    assert_eq!(name_pending(&conn, 100).unwrap(), 1);
    assert_eq!(place(&conn, id).0.as_deref(), Some("Marseille"));
    assert_eq!(name_pending(&conn, 100).unwrap(), 0);
    assert_eq!(hashes(&root), before);
}

#[test]
fn photo_de_telephone_importee_avec_son_lieu() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("telephone");
    fs::create_dir_all(&card).unwrap();
    let taken = NaiveDateTime::parse_from_str("2026-07-14 18:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
    phone_photo(&card.join("IMG_0001.JPG"), taken, "+02:00", 43.2965, 5.3698);
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let id = import::plan(
        &mut conn,
        &ImportOptions {
            library_id: lib.id,
            source_path: card.clone(),
            source_label: "Téléphone".into(),
            files: vec![card.join("IMG_0001.JPG")],
            album_name: "Marseille".into(),
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
    let (media, source): (i64, String) = conn
        .query_row("SELECT id, location_source FROM media", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(source, "exif");
    assert_eq!(place(&conn, media).0.as_deref(), Some("Marseille"));
    assert_eq!(found(&conn, "marseille"), vec![media]);
}

fn times(conn: &Connection, id: i64) -> (String, Option<String>, Option<String>) {
    conn.query_row(
        "SELECT captured_at, utc_offset, captured_at_camera FROM media WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

#[test]
fn heure_du_boitier_corrigee_d_apres_le_lieu() {
    let dir = tempfile::tempdir().unwrap();
    let (mut conn, root, photos) = setup(dir.path(), &["nikon_z6.NEF"]);
    let before = hashes(&root);
    let id = photos[0].id;
    // Le boîtier a écrit 17:13:55 à l'heure de Paris (+01:00) ; la photo est
    // à New York, à l'heure d'hiver (-05:00).
    assert_eq!(times(&conn, id).1.as_deref(), Some("+01:00"));
    let none = time::proposals(&conn, &[id], Clock::Written).unwrap();
    assert_eq!((none.no_place, none.groups.len()), (1, 0));
    locate_manual(&mut conn, &[id], 40.7484, -73.9857, false).unwrap();
    let report = time::proposals(&conn, &[id], Clock::Written).unwrap();
    assert_eq!(report.groups.len(), 1, "{report:?}");
    let g = &report.groups[0];
    assert_eq!(g.city.as_deref(), Some("New York City"));
    assert_eq!(
        (g.camera_minutes, g.local_minutes, g.shift_minutes),
        (60, -300, -360)
    );

    assert_eq!(time::correct(&mut conn, &[id], Clock::Written).unwrap(), 1);
    assert_eq!(
        times(&conn, id),
        (
            "2019-11-29T11:13:55".into(),
            Some("-05:00".into()),
            Some("2019-11-29T17:13:55".into())
        )
    );
    // Déjà à l'heure locale : plus rien à proposer.
    let after = time::proposals(&conn, &[id], Clock::Written).unwrap();
    assert_eq!((after.correct, after.groups.len()), (1, 0));

    // L'heure corrigée part dans le XMP, et la réanalyse la retrouve.
    flush_pending_xmp(&conn, usize::MAX).unwrap();
    let sidecar: String = conn
        .query_row("SELECT sidecar_path FROM media WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap();
    let xmp = read_sidecar(&root.join(sidecar)).unwrap().unwrap();
    assert_eq!(
        xmp.captured_at.as_deref(),
        Some("2019-11-29T11:13:55-05:00")
    );
    let mut fresh = db::open(&dir.path().join("nouvelle.db")).unwrap();
    let lib = library::create_library(&fresh, "Photos", &root, None).unwrap();
    import::rescan::rescan(&mut fresh, lib.id, &AtomicBool::new(false), |_, _| {}).unwrap();
    let again: i64 = fresh
        .query_row("SELECT id FROM media", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        times(&fresh, again),
        (
            "2019-11-29T11:13:55".into(),
            Some("-05:00".into()),
            Some("2019-11-29T17:13:55".into())
        )
    );
    assert_eq!(hashes(&root), before);
}
