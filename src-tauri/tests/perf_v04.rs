//! Critère chiffré de la V0.4 : une journée de 1 000 photos analysée (flou,
//! exposition, yeux fermés) en moins de 10 minutes, en arrière-plan, sans
//! ralentir le tri.
//!
//! Huit échantillons (six RAW, deux portraits) analysés 125 fois chacun par
//! le vrai chemin de l'application, base comprise : depuis les aperçus en
//! cache (le cas courant, après l'import), puis depuis les originaux (le pire
//! cas). La navigation (aperçu et fiche de la photo suivante) est mesurée
//! seule, puis pendant une analyse.
//!
//! `cargo test --release --test perf_v04 -- --ignored --nocapture`

mod common;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use afterdusk_lib::analysis::run;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::media::{
    MediaQuery, cached_preview, generate_previews, list_media, media_details,
};
use afterdusk_lib::{db, library};

const ROUNDS: usize = 125;

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// Passage d'une photo à la suivante, côté cœur : aperçu en cache et fiche.
fn navigate(
    db_path: &std::path::Path,
    cache: &std::path::Path,
    ids: &[i64],
    n: usize,
) -> Vec<Duration> {
    let conn = db::open(db_path).unwrap();
    let mut times = Vec::with_capacity(n);
    for k in 0..n {
        let id = ids[k % ids.len()];
        let t = Instant::now();
        let p = cached_preview(cache, id, 2048).unwrap();
        let bytes = fs::read(p).unwrap();
        assert!(!bytes.is_empty());
        media_details(&conn, id).unwrap().unwrap();
        times.push(t.elapsed());
        // Au rythme d'un photographe qui trie vite.
        std::thread::sleep(Duration::from_millis(40));
    }
    times.sort();
    times
}

#[test]
#[ignore = "mesure de performance : cargo test --release --test perf_v04 -- --ignored"]
fn mille_photos_analysees_sans_ralentir_le_tri() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in common::RAW_SAMPLES
        .iter()
        .chain(&["visage_ouverts.jpg", "visage_fermes.jpg"])
    {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let cache = dir.path().join("cache");
    let db_path = dir.path().join("a.db");
    let mut conn = db::open(&db_path).unwrap();
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
            album_name: "Perf".into(),
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
    let ids: Vec<i64> = list_media(&conn, lib.id, &MediaQuery::default())
        .unwrap()
        .iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids.len(), 8);

    // Les deux cas : sans aperçus (originaux), puis avec (cas courant).
    let analyse_all = |label: &str| {
        let start = Instant::now();
        let mut done = 0;
        for _ in 0..ROUNDS {
            conn.execute("DELETE FROM analysis", []).unwrap();
            let r = run::run(
                &conn,
                &cache,
                lib.id,
                None,
                &AtomicBool::new(false),
                |_, _| {},
            )
            .unwrap();
            assert!(r.eyes, "ONNX Runtime : node scripts/fetch-onnxruntime.mjs");
            done += r.analyzed;
        }
        let total = start.elapsed();
        println!(
            "{label} : {done} photos en {:.1} s, {:.0} ms par photo, 1 000 photos en {:.1} min",
            total.as_secs_f64(),
            1000.0 * total.as_secs_f64() / done as f64,
            total.as_secs_f64() / done as f64 * 1000.0 / 60.0
        );
        total.as_secs_f64() / done as f64 * 1000.0
    };
    let from_originals = analyse_all("depuis les originaux");
    for &id in &ids {
        generate_previews(&conn, &cache, id).unwrap();
    }
    let from_cache = analyse_all("depuis les aperçus");
    assert!(
        from_originals < 600.0 && from_cache < 600.0,
        "plus de 10 minutes pour 1 000 photos"
    );

    // Navigation seule, puis pendant une analyse en continu.
    let alone = navigate(&db_path, &cache, &ids, 100);
    let stop = Arc::new(AtomicBool::new(false));
    let worker = {
        let (stop, db_path, cache) = (stop.clone(), db_path.clone(), cache.clone());
        let lib_id = lib.id;
        std::thread::spawn(move || {
            let conn = db::open(&db_path).unwrap();
            let mut n = 0;
            while !stop.load(Ordering::Relaxed) {
                conn.execute("DELETE FROM analysis", []).unwrap();
                n += run::run(
                    &conn,
                    &cache,
                    lib_id,
                    None,
                    &AtomicBool::new(false),
                    |_, _| {},
                )
                .unwrap()
                .analyzed;
            }
            n
        })
    };
    let busy = navigate(&db_path, &cache, &ids, 100);
    stop.store(true, Ordering::Relaxed);
    let analysed = worker.join().unwrap();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "navigation seule : p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        ms(percentile(&alone, 0.5)),
        ms(percentile(&alone, 0.95)),
        ms(alone[alone.len() - 1])
    );
    println!(
        "navigation pendant l'analyse ({analysed} photos analysées) : p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        ms(percentile(&busy, 0.5)),
        ms(percentile(&busy, 0.95)),
        ms(busy[busy.len() - 1])
    );
}
