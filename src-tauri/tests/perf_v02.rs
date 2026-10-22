//! Critères chiffrés de la V0.2, mesurés sur les vrais RAW de test-assets :
//! - retour visuel d'un réglage en moins de 100 ms ;
//! - export de 100 photos en JPEG 2048 px en moins de 2 minutes.
//!
//! Mesures de performance : à lancer en version optimisée, sur la machine
//! visée (la CI n'en est pas une) :
//! `cargo test --release --test perf_v02 -- --ignored --nocapture`
//! (avec `AFTERDUSK_GPU=0` pour mesurer le rendu processeur).

mod common;

use std::fs;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use afterdusk_lib::develop::gpu::GpuRenderer;
use afterdusk_lib::develop::pipeline::{Clipping, RenderParams, WorkingImage, render};
use afterdusk_lib::develop::settings::{DevelopSettings, WhiteBalance};
use afterdusk_lib::develop::source::PREVIEW_SIDE;
use afterdusk_lib::export::job::{self, ExportRequest, Target};
use afterdusk_lib::export::presets::ExportSettings;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::raw::RawFile;
use afterdusk_lib::{db, library};

/// Tous les réglages en jeu à la fois : le cas le plus lourd d'un curseur.
fn heavy() -> DevelopSettings {
    let mut s = DevelopSettings::default();
    s.light.contrast = 40.0;
    s.light.highlights = -70.0;
    s.light.shadows = 60.0;
    s.light.whites = 20.0;
    s.light.blacks = -25.0;
    s.color.vibrance = 45.0;
    s.color.white_balance = WhiteBalance::Custom {
        temperature: 4200.0,
        tint: 12.0,
    };
    s.curve.rgb = vec![[0.0, 18.0], [70.0, 55.0], [190.0, 205.0], [255.0, 245.0]];
    s.hsl.saturation.blue = 60.0;
    s.hsl.luminance.aqua = -50.0;
    s.effects.vignette.amount = -45.0;
    s.effects.grain.amount = 60.0;
    s
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

#[test]
#[ignore = "mesure de performance : cargo test --release --test perf_v02 -- --ignored"]
fn retour_visuel_d_un_reglage_sous_100_ms() {
    // AFTERDUSK_GPU=0 : rendu processeur, comme l'application sans GPU.
    let gpu = if std::env::var_os("AFTERDUSK_GPU").is_some_and(|v| v == "0") {
        None
    } else {
        GpuRenderer::new().ok()
    };
    println!(
        "rendu : {}",
        gpu.as_ref()
            .map_or("processeur".to_owned(), |g| g.adapter_name().to_owned())
    );
    let mut all = Vec::new();
    for name in common::RAW_SAMPLES {
        let mut raw = RawFile::open(&common::sample(name)).unwrap();
        let image = WorkingImage::from_raw(&raw.develop(true).unwrap(), PREVIEW_SIDE);
        let uploaded = gpu.as_ref().and_then(|g| g.upload(&image).ok());
        let mut settings = heavy();
        let mut times = Vec::new();
        // Un curseur d'exposition que l'on fait glisser : 40 valeurs.
        for step in 0..41 {
            settings.light.exposure = -1.0 + f64::from(step) * 0.05;
            let p =
                RenderParams::from_settings(&settings, image.as_shot_white, Clipping::default());
            let t = Instant::now();
            let frame = match (&gpu, &uploaded) {
                (Some(g), Some(u)) => g.render(u, &p).unwrap(),
                _ => render(&image, &p),
            };
            // Mise en forme pour l'interface : une copie des pixels.
            let bytes = frame.rgba.to_vec();
            assert_eq!(bytes.len(), (image.width * image.height * 4) as usize);
            // La première passe prépare le rendu : elle ne compte pas.
            if step > 0 {
                times.push(t.elapsed());
            }
        }
        times.sort();
        let (median, p95) = (percentile(&times, 0.5), percentile(&times, 0.95));
        println!(
            "{name:22} {}×{} : médiane {:>3} ms, 95 % {:>3} ms",
            image.width,
            image.height,
            median.as_millis(),
            p95.as_millis()
        );
        all.extend(times);
    }
    all.sort();
    let p95 = percentile(&all, 0.95);
    println!("ensemble : 95 % des rendus en {} ms", p95.as_millis());
    assert!(p95 < Duration::from_millis(100), "{} ms", p95.as_millis());
}

#[test]
#[ignore = "mesure de performance : cargo test --release --test perf_v02 -- --ignored"]
fn export_de_100_jpeg_2048_px_sous_2_minutes() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    fs::create_dir_all(&card).unwrap();
    for name in common::RAW_SAMPLES {
        fs::copy(common::sample(name), card.join(name)).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let db_path = dir.path().join("afterdusk.db");
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
            album_name: "Mesure".into(),
            album_per_day: false,
            rename: false,
            backup: false,
            keywords: vec![],
            creator: Some("Lewis Bedar".into()),
            rights: None,
            skip_pair_jpegs: false,
            client_id: None,
            session_type_id: None,
            session_label: String::new(),
        },
    )
    .unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM media ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(ids.len(), common::RAW_SAMPLES.len());
    // 100 photos : les six RAW de test, chacun plusieurs fois.
    let media_ids: Vec<i64> = ids.iter().copied().cycle().take(100).collect();
    let out = dir.path().join("Livraison");
    let started = Instant::now();
    let done = job::run(
        &db_path,
        &ExportSettings::default(),
        &ExportRequest {
            media_ids,
            targets: vec![Target {
                preset: "web".into(),
                folder: None,
            }],
            destination: out.clone(),
        },
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    let elapsed = started.elapsed();
    assert!(done.failures.is_empty(), "{:?}", done.failures);
    let written = fs::read_dir(&out).unwrap().count();
    assert_eq!(written, 100);
    println!(
        "100 JPEG 2048 px en {:.1} s ({} fils de calcul)",
        elapsed.as_secs_f64(),
        std::thread::available_parallelism().map_or(1, |n| n.get())
    );
    assert!(elapsed < Duration::from_secs(120), "{elapsed:?}");
}
