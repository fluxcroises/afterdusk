//! Clients et séances de bout en bout (décision 0009) : import rattaché à un
//! client et à un mariage, rangement `{client}/{seance}`, filtres, recherche
//! par nom de client, puis suppression du client sans toucher aux fichiers.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::clients::{self, ClientInput};
use afterdusk_lib::library::filter::MediaFilter;
use afterdusk_lib::library::media::{MediaQuery, list_albums, list_media};
use afterdusk_lib::{db, library};

#[test]
fn import_rattache_filtres_et_suppression() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    let names = &common::RAW_SAMPLES[..2];
    for name in names {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let originals: Vec<_> = names
        .iter()
        .map(|n| common::sha256(&card.join(n)))
        .collect();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("a.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    library::set_templates(&conn, lib.id, "{client}/{seance} {album}", None).unwrap();

    let client = clients::create(
        &conn,
        &ClientInput {
            name: "Famille Dupont".into(),
            email: Some("dupont@example.com".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let wedding = clients::session_types(&conn).unwrap()[0].id;
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
            album_name: "Église".into(),
            album_per_day: false,
            rename: false,
            backup: false,
            keywords: vec![],
            creator: None,
            rights: None,
            skip_pair_jpegs: false,
            client_id: Some(client),
            session_type_id: Some(wedding),
            session_label: "Mariage".into(),
        },
    )
    .unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();

    // Rangement par client puis séance ; l'album porte le client et la séance.
    let albums = list_albums(&conn, lib.id).unwrap();
    assert_eq!(albums.len(), 1);
    let album = &albums[0];
    assert_eq!(album.folder, "Famille Dupont/Mariage Église");
    assert_eq!(
        (
            album.client_id,
            album.client_name.as_deref(),
            album.session_type_id
        ),
        (Some(client), Some("Famille Dupont"), Some(wedding))
    );
    assert!(root.join("Famille Dupont/Mariage Église").is_dir());

    let count = |f: MediaFilter| {
        list_media(
            &conn,
            lib.id,
            &MediaQuery {
                filter: f,
                ..Default::default()
            },
        )
        .unwrap()
        .len()
    };
    assert_eq!(
        count(MediaFilter {
            client_id: Some(client),
            ..Default::default()
        }),
        2
    );
    assert_eq!(
        count(MediaFilter {
            session_type_id: Some(wedding + 1),
            ..Default::default()
        }),
        0
    );
    // La recherche trouve les photos par le nom du client.
    assert_eq!(
        count(MediaFilter {
            query: Some("dupont".into()),
            ..Default::default()
        }),
        2
    );

    // Supprimer le client : fiche effacée, album et fichiers intacts.
    let stored: Vec<_> = fs::read_dir(root.join("Famille Dupont/Mariage Église"))
        .unwrap()
        .map(|e| common::sha256(&e.unwrap().path()))
        .collect();
    clients::delete(&conn, client).unwrap();
    assert!(clients::list(&conn).unwrap().is_empty());
    let album = &list_albums(&conn, lib.id).unwrap()[0];
    assert_eq!((album.client_id, album.count), (None, 2));
    assert_eq!(count(MediaFilter::default()), 2);
    let after: Vec<_> = fs::read_dir(root.join("Famille Dupont/Mariage Église"))
        .unwrap()
        .map(|e| common::sha256(&e.unwrap().path()))
        .collect();
    assert_eq!(stored, after);
    // Règle absolue : la carte n'a pas changé.
    let now: Vec<_> = names
        .iter()
        .map(|n| common::sha256(&card.join(n)))
        .collect();
    assert_eq!(originals, now);
}
