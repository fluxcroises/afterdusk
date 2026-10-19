//! Filtres et recherche de la Bibliothèque, sur les six RAW de test-assets.

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::db;
use afterdusk_lib::import::{self, ImportOptions, scan};
use afterdusk_lib::library::filter::{
    MediaFilter, MediaSort, RatingFilter, RatingOp, filter_options,
};
use afterdusk_lib::library::marks::{Mark, apply_mark};
use afterdusk_lib::library::media::{MediaQuery, list_media};
use afterdusk_lib::library::{self, trash};
use afterdusk_lib::metadata::xmp::{ColorLabel, Flag};
use rusqlite::Connection;

const SAMPLES: [&str; 6] = [
    "canon_eos_r6.CR3",
    "fujifilm_x-t3.RAF",
    "nikon_z6.NEF",
    "panasonic_dc-g9.RW2",
    "ricoh_gr-iii.DNG",
    "sony_ilce-7m3.ARW",
];

struct Setup {
    _dir: tempfile::TempDir,
    conn: Connection,
    library_id: i64,
}

impl Setup {
    fn id(&self, name: &str) -> i64 {
        self.conn
            .query_row(
                "SELECT id FROM media WHERE path LIKE '%' || ?1",
                [name],
                |r| r.get(0),
            )
            .unwrap()
    }

    /// Noms de fichiers que renvoie le filtre, dans l'ordre de la grille.
    fn names(&self, filter: MediaFilter, sort: MediaSort) -> Vec<String> {
        let query = MediaQuery {
            filter,
            sort,
            ..Default::default()
        };
        list_media(&self.conn, self.library_id, &query)
            .unwrap()
            .into_iter()
            .map(|m| m.file_name)
            .collect()
    }

    fn filtered(&self, filter: MediaFilter) -> Vec<String> {
        let mut names = self.names(filter, MediaSort::FileName);
        names.sort();
        names
    }
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in SAMPLES {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let files = scan::scan(&card)
        .unwrap()
        .files
        .into_iter()
        .map(|f| f.path)
        .collect();
    let opts = ImportOptions {
        library_id: lib.id,
        source_path: card,
        source_label: "Carte".into(),
        files,
        album_name: "Été à Cabourg".into(),
        album_per_day: false,
        rename: false,
        backup: false,
        keywords: vec!["Normandie".into()],
        creator: None,
        rights: None,
    };
    let id = import::plan(&mut conn, &opts).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let s = Setup {
        _dir: dir,
        conn,
        library_id: lib.id,
    };
    let mut conn = s.conn;
    let ids = |n: &[&str]| -> Vec<i64> {
        n.iter()
            .map(|name| {
                conn.query_row(
                    "SELECT id FROM media WHERE path LIKE '%' || ?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap()
            })
            .collect()
    };
    let (five, three, picked, rejected, red) = (
        ids(&["canon_eos_r6.CR3"]),
        ids(&["nikon_z6.NEF", "sony_ilce-7m3.ARW"]),
        ids(&["canon_eos_r6.CR3", "nikon_z6.NEF"]),
        ids(&["fujifilm_x-t3.RAF"]),
        ids(&["sony_ilce-7m3.ARW"]),
    );
    apply_mark(&mut conn, &five, Mark::Rating(5)).unwrap();
    apply_mark(&mut conn, &three, Mark::Rating(3)).unwrap();
    apply_mark(&mut conn, &picked, Mark::Flag(Flag::Picked)).unwrap();
    apply_mark(&mut conn, &rejected, Mark::Flag(Flag::Rejected)).unwrap();
    apply_mark(&mut conn, &red, Mark::Color(Some(ColorLabel::Red))).unwrap();
    Setup { conn, ..s }
}

#[test]
fn note_drapeau_couleur() {
    let s = setup();
    let at_least_3 = MediaFilter {
        rating: Some(RatingFilter {
            op: RatingOp::AtLeast,
            value: 3,
        }),
        ..Default::default()
    };
    assert_eq!(
        s.filtered(at_least_3.clone()),
        ["canon_eos_r6.CR3", "nikon_z6.NEF", "sony_ilce-7m3.ARW"]
    );
    let unrated = MediaFilter {
        rating: Some(RatingFilter {
            op: RatingOp::Exactly,
            value: 0,
        }),
        ..Default::default()
    };
    assert_eq!(s.filtered(unrated).len(), 3);
    // Les filtres se combinent.
    let picked_3 = MediaFilter {
        flags: vec![1],
        ..at_least_3
    };
    assert_eq!(s.filtered(picked_3), ["canon_eos_r6.CR3", "nikon_z6.NEF"]);
    let not_rejected = MediaFilter {
        flags: vec![0, 1],
        ..Default::default()
    };
    assert_eq!(s.filtered(not_rejected).len(), 5);
    let red_only = MediaFilter {
        colors: vec!["red".into()],
        ..Default::default()
    };
    assert_eq!(s.filtered(red_only), ["sony_ilce-7m3.ARW"]);
    let uncolored = MediaFilter {
        colors: vec!["none".into()],
        ..Default::default()
    };
    assert_eq!(s.filtered(uncolored).len(), 5);
}

#[test]
fn boitier_objectif_et_periode() {
    let s = setup();
    let options = filter_options(&s.conn, s.library_id).unwrap();
    assert_eq!(options.cameras.len(), 6, "{:?}", options.cameras);
    let sony = options
        .cameras
        .iter()
        .find(|c| c.name.contains("ILCE-7M3"))
        .unwrap();
    assert_eq!(sony.count, 1);
    let by_camera = MediaFilter {
        camera_id: Some(sony.id),
        ..Default::default()
    };
    assert_eq!(s.filtered(by_camera), ["sony_ilce-7m3.ARW"]);

    let lens = options.lenses.first().expect("au moins un objectif lu");
    let by_lens = MediaFilter {
        lens_id: Some(lens.id),
        ..Default::default()
    };
    assert_eq!(s.filtered(by_lens).len() as i64, lens.count);

    // Période : le premier jour seul, puis toute l'étendue.
    let first = options.first_day.clone().unwrap();
    let last = options.last_day.clone().unwrap();
    assert!(first < last);
    let first_day_only = MediaFilter {
        from: Some(first.clone()),
        to: Some(first),
        ..Default::default()
    };
    let n = s.filtered(first_day_only).len();
    assert!((1..6).contains(&n));
    let all = MediaFilter {
        from: options.first_day,
        to: Some(last),
        ..Default::default()
    };
    assert_eq!(s.filtered(all).len(), 6);
}

#[test]
fn recherche_texte() {
    let s = setup();
    let search = |q: &str| {
        s.filtered(MediaFilter {
            query: Some(q.into()),
            ..Default::default()
        })
    };
    // Nom de fichier, entier ou en partie.
    assert_eq!(search("nikon"), ["nikon_z6.NEF"]);
    assert_eq!(search("7m3"), ["sony_ilce-7m3.ARW"]);
    // Album et mots-clés, sans tenir compte des accents ni de la casse.
    assert_eq!(search("ete cabourg").len(), 6);
    assert_eq!(search("normandie").len(), 6);
    assert_eq!(search("NORMAND").len(), 6);
    assert!(search("Lisbonne").is_empty());
    // Caractères spéciaux : jamais d'erreur SQL ni FTS.
    assert!(search("\"*(OR)%_").is_empty());
}

#[test]
fn tri_et_corbeille() {
    let mut s = setup();
    let by_rating = s.names(MediaFilter::default(), MediaSort::Rating);
    assert_eq!(by_rating[0], "canon_eos_r6.CR3");
    let by_name = s.names(MediaFilter::default(), MediaSort::FileName);
    assert_eq!(by_name, SAMPLES);
    // Une photo de la corbeille sort des grilles et des choix de boîtiers.
    let sony = s.id("sony_ilce-7m3.ARW");
    trash::move_to_trash(&mut s.conn, &[sony]).unwrap();
    assert_eq!(
        s.names(MediaFilter::default(), MediaSort::FileName).len(),
        5
    );
    let options = filter_options(&s.conn, s.library_id).unwrap();
    assert!(!options.cameras.iter().any(|c| c.name.contains("ILCE-7M3")));
}
