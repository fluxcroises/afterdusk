//! Collections de bout en bout sur les six RAW de test : collection
//! manuelle (ajout, doublon ignoré, retrait, corbeille), album intelligent
//! « 5 étoiles à 52 mm » recalculé à chaque lecture, filtre par focale.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::collections::{self, CollectionKind};
use afterdusk_lib::library::filter::{self, MediaFilter, RatingFilter, RatingOp};
use afterdusk_lib::library::marks::{Mark, apply_mark};
use afterdusk_lib::library::media::{MediaQuery, list_media};
use afterdusk_lib::library::trash;
use afterdusk_lib::library::{LibraryError, LibraryError::*};
use afterdusk_lib::{db, library};

#[test]
fn collections_manuelles_et_albums_intelligents() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in common::RAW_SAMPLES {
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
        },
    )
    .unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();

    let all = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    assert_eq!(all.len(), 6);
    let by_name = |part: &str| {
        all.iter()
            .find(|m| m.file_name.to_lowercase().contains(part))
            .unwrap()
            .id
    };
    let (nikon, sony, canon) = (by_name("nikon"), by_name("sony"), by_name("canon"));
    let in_filter = |conn: &rusqlite::Connection, f: MediaFilter| {
        let q = MediaQuery {
            filter: f,
            ..Default::default()
        };
        let mut ids: Vec<i64> = list_media(conn, lib.id, &q)
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        ids.sort();
        ids
    };

    // Focales proposées par la barre de filtres, et filtre sur l'une d'elles.
    let options = filter::filter_options(&conn, lib.id).unwrap();
    assert!(
        options.focals.iter().any(|f| f.value == 52.0),
        "{:?}",
        options.focals
    );
    let at_52 = MediaFilter {
        focal: Some([52.0, 52.0]),
        ..Default::default()
    };
    assert!(in_filter(&conn, at_52.clone()).contains(&nikon));

    // Album intelligent : 5 étoiles à 52 mm, vide tant que rien n'est noté.
    let five_at_52 = MediaFilter {
        rating: Some(RatingFilter {
            op: RatingOp::Exactly,
            value: 5,
        }),
        ..at_52
    };
    let smart =
        collections::create(&conn, lib.id, "  5 étoiles à 52 mm ", Some(&five_at_52)).unwrap();
    let count_of = |conn: &rusqlite::Connection, id: i64| {
        collections::list(conn, lib.id)
            .unwrap()
            .into_iter()
            .find(|c| c.id == id)
            .unwrap()
            .count
    };
    assert_eq!(count_of(&conn, smart), 0);
    apply_mark(&mut conn, &[nikon, sony], Mark::Rating(5)).unwrap();
    // Recalculé à la lecture : la photo notée y entre aussitôt.
    assert_eq!(count_of(&conn, smart), 1);

    // Collection manuelle : doublon et identifiant inconnu ignorés.
    let manual = collections::create(&conn, lib.id, "Sélection", None).unwrap();
    assert_eq!(
        collections::add_media(&mut conn, manual, &[nikon, sony, nikon, 999_999]).unwrap(),
        2
    );
    assert_eq!(
        collections::add_media(&mut conn, manual, &[canon]).unwrap(),
        1
    );
    let in_manual = MediaFilter {
        collection_id: Some(manual),
        ..Default::default()
    };
    let mut expected = vec![nikon, sony, canon];
    expected.sort();
    assert_eq!(in_filter(&conn, in_manual.clone()), expected);
    collections::remove_media(&conn, manual, &[sony]).unwrap();
    assert_eq!(count_of(&conn, manual), 2);

    // Liste par nom, avec le type et le filtre des seuls albums intelligents.
    let list = collections::list(&conn, lib.id).unwrap();
    assert_eq!(
        list.iter()
            .map(|c| (c.name.as_str(), c.kind))
            .collect::<Vec<_>>(),
        [
            ("5 étoiles à 52 mm", CollectionKind::Smart),
            ("Sélection", CollectionKind::Manual)
        ]
    );
    assert!(list[1].filter.is_none());
    assert_eq!(
        list[0].filter.as_ref().and_then(|f| f.focal),
        Some([52.0, 52.0])
    );

    // Corbeille : la photo sort des deux, et y revient restaurée.
    trash::move_to_trash(&mut conn, &[nikon]).unwrap();
    assert_eq!((count_of(&conn, manual), count_of(&conn, smart)), (1, 0));
    trash::restore(&mut conn, &[nikon]).unwrap();
    assert_eq!((count_of(&conn, manual), count_of(&conn, smart)), (2, 1));

    // Chaque type refuse l'opération de l'autre ; un nom vide est refusé.
    assert!(matches!(
        collections::add_media(&mut conn, smart, &[canon]),
        Err(NotManual(_))
    ));
    assert!(matches!(
        collections::set_filter(&conn, manual, &MediaFilter::default()),
        Err(NotSmart(_))
    ));
    assert!(matches!(
        collections::create(&conn, lib.id, "   ", None),
        Err(LibraryError::EmptyName)
    ));

    // Remplacer le filtre, renommer, supprimer : les photos ne bougent pas.
    collections::set_filter(&conn, smart, &MediaFilter::default()).unwrap();
    assert_eq!(count_of(&conn, smart), 6);
    collections::rename(&conn, manual, "Portfolio").unwrap();
    collections::delete(&conn, manual).unwrap();
    collections::delete(&conn, smart).unwrap();
    assert!(collections::list(&conn, lib.id).unwrap().is_empty());
    assert_eq!(
        list_media(&conn, lib.id, &MediaQuery::default())
            .unwrap()
            .len(),
        6
    );
    assert!(matches!(
        collections::delete(&conn, manual),
        Err(CollectionNotFound(_))
    ));
}
