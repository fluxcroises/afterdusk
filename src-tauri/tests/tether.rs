//! Connexion au boîtier de bout en bout avec le boîtier factice (décision
//! 0016) : la photo déclenchée est reçue, importée dans l'album de la séance
//! en moins de 5 secondes, la copie de réception est retirée, et la carte du
//! boîtier n'a pas bougé.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use afterdusk_lib::import::auto::AutoSettings;
use afterdusk_lib::import::watch;
use afterdusk_lib::library::media::{MediaQuery, list_albums, list_media};
use afterdusk_lib::tether::fake::FakeCamera;
use afterdusk_lib::tether::ptp::{self, Camera};
use afterdusk_lib::tether::session::{Session, Update};
use afterdusk_lib::{db, library, tether};

#[test]
fn photo_declenchee_importee_dans_l_album_de_la_seance() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("a.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let reception = tether::reception_dir(dir.path());
    let album = "X-H2 · 31/10/2026";
    watch::ensure_reception(
        &conn,
        &reception,
        lib.id,
        album,
        &AutoSettings::default(),
        true,
    )
    .unwrap();
    let folder = watch::list(&conn)
        .unwrap()
        .into_iter()
        .find(|f| f.path == reception)
        .unwrap();

    let mut session = Session::open(Camera::new(FakeCamera::new(&[], 30)), &reception).unwrap();
    session.poll(&conn).unwrap();

    let raw = fs::read(common::sample(common::RAW_SAMPLES[0])).unwrap();
    let start = Instant::now();
    session
        .camera()
        .transport()
        .shoot("DSCF0001.RAF", raw.clone());
    let updates = session.poll(&conn).unwrap();
    let received: Vec<_> = updates
        .iter()
        .filter_map(|u| match u {
            Update::Received { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(received.len(), 1);
    let imported = watch::import_files(
        &mut conn,
        &folder,
        received.clone(),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap()
    .unwrap();
    let elapsed = start.elapsed();
    assert_eq!(imported.media_ids.len(), 1);
    assert!(
        elapsed < Duration::from_secs(5),
        "photo affichable après {elapsed:?}"
    );
    println!("boîtier relié : photo importée en {elapsed:?}");
    let albums = list_albums(&conn, lib.id).unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].name, album);
    assert_eq!(
        list_media(&conn, lib.id, &MediaQuery::default())
            .unwrap()
            .len(),
        1
    );

    // Copie de réception retirée une fois la photo dans la bibliothèque.
    assert_eq!(watch::remove_imported(&conn, lib.id, &received).unwrap(), 1);
    assert!(!received[0].exists());

    // Règles absolues : la carte n'a pas bougé, seules des lectures sont
    // parties vers le boîtier.
    let fake = session.camera().transport();
    assert_eq!(fake.card(), vec![("DSCF0001.RAF".to_owned(), raw)]);
    assert!(fake.sent().iter().all(|op| ptp::ALLOWED.contains(op)));
}
