//! Rafales de bout en bout : trois prises du Nikon (même image, même instant)
//! forment une rafale, les autres boîtiers restent seuls. Pile repliée dans
//! la grille, dépliée sur demande, photo du dessus, retrait, empilage à la
//! main, corbeille. Les originaux ne sont que lus.

mod common;

use std::fs;
use std::io::Write;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::bursts;
use afterdusk_lib::library::filter::MediaFilter;
use afterdusk_lib::library::marks::{Mark, apply_mark};
use afterdusk_lib::library::media::{MediaQuery, MediaSummary, list_media};
use afterdusk_lib::library::trash;
use afterdusk_lib::{db, library};

fn names(list: &[MediaSummary]) -> Vec<String> {
    list.iter().map(|m| m.file_name.clone()).collect()
}

#[test]
fn rafale_detectee_repliee_et_modifiable() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    // Trois « déclenchements » : le même NEF, quelques octets différents en
    // fin de fichier pour que l'import ne les prenne pas pour des doublons.
    for (i, name) in ["DSC_0001.NEF", "DSC_0002.NEF", "DSC_0003.NEF"]
        .iter()
        .enumerate()
    {
        let target = card.join(name);
        fs::copy(common::sample("nikon_z6.NEF"), &target).unwrap();
        let mut f = fs::OpenOptions::new().append(true).open(&target).unwrap();
        f.write_all(&[b'x'; 16][..=i]).unwrap();
    }
    fs::copy(common::sample("sony_ilce-7m3.ARW"), card.join("sony.ARW")).unwrap();
    let before = common::sha256(&card.join("DSC_0002.NEF"));

    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let cache = dir.path().join("cache");
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
            album_name: "Rafale".into(),
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
    assert_eq!(all.len(), 4, "{:?}", names(&all));
    let id_of = |name: &str| all.iter().find(|m| m.file_name == name).unwrap().id;
    let (one, two, three) = (
        id_of("DSC_0001.NEF"),
        id_of("DSC_0002.NEF"),
        id_of("DSC_0003.NEF"),
    );

    // La mieux notée passe au-dessus.
    apply_mark(&mut conn, &[two], Mark::Rating(4)).unwrap();
    let mut calls = 0;
    let report = bursts::detect(&mut conn, &cache, lib.id, None, |_, _| calls += 1).unwrap();
    assert_eq!((report.bursts, report.photos), (1, 3));
    assert_eq!(
        calls, 3,
        "hash calculé pour les trois prises voisines seulement"
    );
    // Le hash lit l'original sans l'écrire, et la carte n'est pas touchée.
    let rel: String = conn
        .query_row("SELECT path FROM media WHERE id = ?1", [two], |r| r.get(0))
        .unwrap();
    assert_eq!(common::sha256(&root.join(rel)), before);
    assert_eq!(common::sha256(&card.join("DSC_0002.NEF")), before);

    // Repliée : la photo du dessus seule, avec la taille de la rafale.
    let grid = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    assert_eq!(grid.len(), 2, "{:?}", names(&grid));
    let top = grid.iter().find(|m| m.burst_id.is_some()).unwrap();
    assert_eq!((top.id, top.burst_size), (two, 3));
    // Un filtre montre chaque photo qui y répond.
    let any_flag = MediaQuery {
        filter: MediaFilter {
            flags: vec![0],
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(list_media(&conn, lib.id, &any_flag).unwrap().len(), 4);
    // Dépliée : les autres photos suivent celle du dessus.
    let open = MediaQuery {
        expanded: vec![two],
        ..Default::default()
    };
    let rows = list_media(&conn, lib.id, &open).unwrap();
    let pos = |id: i64| rows.iter().position(|m| m.id == id).unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!((pos(one), pos(three)), (pos(two) + 1, pos(two) + 2));

    // Nouvelle détection : rien ne bouge, le dessus choisi reste.
    assert!(bursts::set_top(&conn, three).unwrap());
    bursts::detect(&mut conn, &cache, lib.id, None, |_, _| {}).unwrap();
    let grid = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    assert!(grid.iter().any(|m| m.id == three && m.burst_size == 3));

    // Corbeille : le dessus jeté, la suivante prend sa place.
    trash::move_to_trash(&mut conn, &[three]).unwrap();
    let grid = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    let visible = grid.iter().find(|m| m.burst_id.is_some()).unwrap();
    assert_eq!((visible.id, visible.burst_size), (one, 2));
    trash::restore(&mut conn, &[three]).unwrap();

    // Retrait : la rafale devient manuelle, la détection ne la recalcule plus.
    assert_eq!(bursts::unstack(&mut conn, &[one]).unwrap(), 1);
    bursts::detect(&mut conn, &cache, lib.id, None, |_, _| {}).unwrap();
    let grid = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    assert_eq!(grid.len(), 3, "{:?}", names(&grid));
    // Deux photos retirées : il n'en reste qu'une, la rafale disparaît.
    bursts::unstack(&mut conn, &[two]).unwrap();
    let grid = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    assert!(grid.iter().all(|m| m.burst_id.is_none()));

    // Empilage à la main, même entre boîtiers ; une seule photo ne suffit pas.
    let sony = id_of("sony.ARW");
    assert!(bursts::stack(&mut conn, lib.id, &[sony]).unwrap().is_none());
    assert!(
        bursts::stack(&mut conn, lib.id, &[sony, one])
            .unwrap()
            .is_some()
    );
    let grid = list_media(&conn, lib.id, &MediaQuery::default()).unwrap();
    assert_eq!(grid.len(), 3);
    assert!(grid.iter().any(|m| m.id == sony && m.burst_size == 2));
}
