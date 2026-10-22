//! Import automatique d'une carte connue (décision 0009) : règle créée
//! d'après un import, carte déjà importée ignorée, nouvelle photo du même
//! boîtier importée seule, carte d'un autre boîtier laissée de côté, et
//! aucune écriture sur la carte.

mod common;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use afterdusk_lib::import::auto::{self, AutoSettings, CardPlan};
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::media::{MediaQuery, list_media};
use afterdusk_lib::{db, library};

fn card(dir: &Path, name: &str, files: &[&str]) -> PathBuf {
    let dcim = dir.join(name).join("DCIM").join("100CAMRA");
    fs::create_dir_all(&dcim).unwrap();
    for f in files {
        fs::copy(common::sample(f), dcim.join(f)).unwrap();
    }
    dir.join(name)
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
fn carte_connue_importee_seule() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("a.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();

    // Import manuel d'une carte : les boîtiers à numéro de série sont proposés.
    let first = card(dir.path(), "carte1", &common::RAW_SAMPLES);
    let files: Vec<PathBuf> = hashes(&first).into_iter().map(|(p, _)| p).collect();
    let id = import::plan(
        &mut conn,
        &ImportOptions {
            library_id: lib.id,
            source_path: first.clone(),
            source_label: "Carte (E:)".into(),
            files,
            album_name: "Mariage Dupont".into(),
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
    let bodies = auto::candidates(&conn, id).unwrap();
    assert!(!bodies.is_empty(), "aucun échantillon avec numéro de série");
    let settings = AutoSettings {
        album_per_day: true,
        keywords: vec!["auto".into()],
        ..Default::default()
    };
    assert_eq!(
        auto::add_from_import(&conn, id, &settings).unwrap(),
        bodies.len()
    );
    // Plus rien à proposer pour cet import.
    assert!(auto::candidates(&conn, id).unwrap().is_empty());
    let rules = auto::list(&conn).unwrap();
    assert_eq!(rules[0].settings, settings);

    // La même carte, réinsérée : tout est déjà là.
    assert!(matches!(
        auto::plan_for_card(&conn, &first, "Carte (E:)").unwrap(),
        CardPlan::NothingNew { .. }
    ));

    // Une nouvelle photo d'un boîtier connu (fin du fichier différente, donc
    // une autre empreinte), à côté d'une photo déjà importée.
    let known_body = rules[0].body_serial.clone();
    let sample = common::RAW_SAMPLES
        .iter()
        .copied()
        .find(|name| {
            let m = afterdusk_lib::metadata::media::read_metadata(
                &common::sample(name),
                afterdusk_lib::metadata::media::MediaFormat::Raw,
            );
            m.body_serial.as_deref().map(str::trim) == Some(known_body.as_str())
        })
        .unwrap();
    let second = card(dir.path(), "carte2", &[sample]);
    let fresh = second.join("DCIM/100CAMRA").join(sample);
    fs::OpenOptions::new()
        .append(true)
        .open(&fresh)
        .unwrap()
        .write_all(b"nouvelle prise")
        .unwrap();
    let before = hashes(&second);
    let plan = auto::plan_for_card(&conn, &second, "Carte (F:)").unwrap();
    let CardPlan::Import {
        options, rule_id, ..
    } = plan
    else {
        panic!("la carte du boîtier connu doit partir à l'import");
    };
    assert_eq!(options.files, vec![fresh.clone()]);
    assert!(options.album_per_day && options.album_name.is_empty());
    assert_eq!(options.keywords, vec!["auto".to_owned()]);
    let count = list_media(&conn, lib.id, &MediaQuery::default())
        .unwrap()
        .len();
    let id = import::plan(&mut conn, &options).unwrap();
    auto::touch(&conn, rule_id).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(
        list_media(&conn, lib.id, &MediaQuery::default())
            .unwrap()
            .len(),
        count + 1
    );
    let used = auto::list(&conn).unwrap();
    assert!(
        used.iter()
            .find(|r| r.id == rule_id)
            .unwrap()
            .last_used_at
            .is_some()
    );
    // Règle absolue : rien n'a été écrit sur la carte.
    assert_eq!(before, hashes(&second));

    // Sans règle, la carte redevient inconnue.
    for rule in auto::list(&conn).unwrap() {
        auto::delete(&conn, rule.id).unwrap();
    }
    assert!(matches!(
        auto::plan_for_card(&conn, &second, "Carte (F:)").unwrap(),
        CardPlan::Unknown
    ));
}
