//! Critère de la V0.1 : import de 500 RAW avec rangement, renommage,
//! sauvegarde et vérification par somme de contrôle, sans aucune perte,
//! coupure comprise. L'import tourne dans un processus séparé, tué net en
//! plein travail (comme une coupure de courant), puis reprend.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime};

use afterdusk_lib::db;
use afterdusk_lib::import::copy::{PART_SUFFIX, hash_file};
use afterdusk_lib::import::{self, ImportOptions, ImportOutcome, scan};
use afterdusk_lib::library::{self, paths};
use afterdusk_lib::metadata::media::format_of;

const FILES: usize = 500;
const SIZE: usize = 1 << 20;
const EXTENSIONS: [&str; 6] = ["RAF", "CR3", "NEF", "ARW", "RW2", "DNG"];
const CHILD_DB: &str = "AFTERDUSK_TEST_CHILD_DB";
const CHILD_IMPORT: &str = "AFTERDUSK_TEST_CHILD_IMPORT";

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, (String, SystemTime)> {
    walk(dir)
        .into_iter()
        .map(|p| {
            let modified = fs::metadata(&p).unwrap().modified().unwrap();
            (p.clone(), (hash_file(&p).unwrap(), modified))
        })
        .collect()
}

/// Carte de 500 « RAW » de 1 Mo, contenus tous différents, prises de vue
/// réparties sur trois jours (date de modification, faute d'EXIF).
fn make_card(card: &Path) -> Vec<PathBuf> {
    let dcim = card.join("DCIM").join("100_TEST");
    fs::create_dir_all(&dcim).unwrap();
    let start = SystemTime::now() - Duration::from_secs(10 * 86_400);
    (0..FILES)
        .map(|i| {
            let ext = EXTENSIONS[i % EXTENSIONS.len()];
            let path = dcim.join(format!("IMG_{i:04}.{ext}"));
            let mut state = (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let data: Vec<u8> = (0..SIZE)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            fs::write(&path, data).unwrap();
            let taken = start + Duration::from_secs((i / 170) as u64 * 86_400 + i as u64 * 60);
            fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(taken)
                .unwrap();
            path
        })
        .collect()
}

/// Processus enfant : l'import lui-même, que le parent tue en cours de route.
#[test]
#[ignore = "lancé par import_500_coupure_et_reprise dans un processus séparé"]
fn processus_d_import() {
    let (Some(db_path), Some(id)) = (std::env::var_os(CHILD_DB), std::env::var_os(CHILD_IMPORT))
    else {
        return;
    };
    let id: i64 = id.to_string_lossy().parse().unwrap();
    let mut conn = db::open(Path::new(&db_path)).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
}

#[test]
fn import_500_coupure_et_reprise() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    let files = make_card(&card);
    let root = dir.path().join("Bibliotheque");
    let backup = dir.path().join("Sauvegarde");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&backup).unwrap();
    let db_path = dir.path().join("afterdusk.db");
    let mut conn = db::open(&db_path).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, Some(&backup)).unwrap();
    let before = snapshot(&card);
    let sources: BTreeMap<String, String> = files
        .iter()
        .map(|p| (p.to_string_lossy().into_owned(), hash_file(p).unwrap()))
        .collect();

    let opts = ImportOptions {
        library_id: lib.id,
        source_path: card.clone(),
        source_label: "Carte".into(),
        files: files.clone(),
        album_name: "Sortie".into(),
        album_per_day: true,
        rename: true,
        backup: true,
        keywords: vec![],
        creator: None,
        rights: None,
        skip_pair_jpegs: false,
        client_id: None,
        session_type_id: None,
        session_label: String::new(),
    };
    let id = import::plan(&mut conn, &opts).unwrap();
    drop(conn);

    // Coupure : le processus d'import est tué net une fois le tiers passé.
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "processus_d_import", "--ignored", "--nocapture"])
        .env(CHILD_DB, &db_path)
        .env(CHILD_IMPORT, id.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let watch = db::connect(&db_path).unwrap();
    let deadline = Instant::now() + Duration::from_secs(300);
    let done_at_cut = loop {
        let done: i64 = watch
            .query_row(
                "SELECT count(*) FROM import_items WHERE import_id = ?1 AND status = 'done'",
                [id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if done >= (FILES / 3) as i64 {
            child.kill().unwrap();
            break done;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "l'import s'est terminé avant la coupure : rien n'a été coupé"
        );
        assert!(Instant::now() < deadline, "import trop lent");
        std::thread::sleep(Duration::from_millis(10));
    };
    child.wait().unwrap();
    drop(watch);
    assert!(done_at_cut < FILES as i64);

    // Redémarrage : l'import coupé est proposé à la reprise, puis repris.
    let mut conn = db::open(&db_path).unwrap();
    import::mark_interrupted(&conn).unwrap();
    let resumable = import::resumable_imports(&conn).unwrap();
    assert_eq!(resumable.len(), 1);
    assert!(resumable[0].remaining > 0);
    let outcome = import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(outcome, ImportOutcome::Done);

    // Aucune perte : 500 photos, copie et sauvegarde identiques à l'original.
    let rows: Vec<(String, String, String)> = conn
        .prepare(
            "SELECT t.source_path, t.target_path, m.content_hash FROM import_items t
             JOIN media m ON m.id = t.media_id WHERE t.import_id = ?1 AND t.status = 'done'",
        )
        .unwrap()
        .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.len(), FILES);
    let mut targets = BTreeSet::new();
    let mut numbers = BTreeSet::new();
    for (source, target, hash) in &rows {
        assert_eq!(&sources[source], hash, "{source}");
        let copy = paths::to_absolute(&root, target).unwrap();
        assert_eq!(&hash_file(&copy).unwrap(), hash, "{target}");
        let saved = paths::to_absolute(&backup, target).unwrap();
        assert_eq!(&hash_file(&saved).unwrap(), hash, "sauvegarde de {target}");
        assert!(
            targets.insert(target.clone()),
            "deux photos au même endroit : {target}"
        );
        // Renommage AAAAMMJJ_HHMMSS_boîtier_NNNN : un numéro par photo, sans trou.
        let stem = Path::new(target)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        numbers.insert(stem.rsplit('_').next().unwrap().to_owned());
    }
    let expected: BTreeSet<String> = (1..=FILES).map(|n| format!("{n:04}")).collect();
    assert_eq!(numbers, expected);
    // Rangement : un album par jour de prise de vue (lu comme le fait l'import).
    let days: BTreeSet<String> = files
        .iter()
        .map(|f| {
            let format = format_of(f).unwrap();
            scan::describe(f, format).unwrap().day().to_owned()
        })
        .collect();
    assert!(days.len() >= 3);
    let albums: i64 = conn
        .query_row("SELECT count(*) FROM albums", [], |r| r.get(0))
        .unwrap();
    assert_eq!(albums, days.len() as i64);
    let media: i64 = conn
        .query_row("SELECT count(*) FROM media", [], |r| r.get(0))
        .unwrap();
    assert_eq!(media, FILES as i64);
    // Aucun fichier temporaire laissé par la coupure.
    for dir in [&root, &backup] {
        let leftovers: Vec<PathBuf> = walk(dir)
            .into_iter()
            .filter(|p| p.to_string_lossy().ends_with(PART_SUFFIX))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
    // Règle absolue : la carte n'a pas bougé.
    assert_eq!(snapshot(&card), before);
}
