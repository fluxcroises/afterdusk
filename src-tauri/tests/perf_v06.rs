//! Critère chiffré de la V0.6 : retour visuel d'un réglage en moins de
//! 100 ms avec 8 masques, mesuré comme celui de la V0.2 sur les vrais RAW de
//! test-assets (tous les réglages de la photo en jeu, plus quatre dégradés
//! linéaires et quatre radiaux, chacun avec plusieurs réglages locaux) : on
//! fait glisser l'exposition d'un masque.
//!
//! `cargo test --release --test perf_v06 -- --ignored --nocapture`
//! (avec `AFTERDUSK_GPU=0` pour mesurer le rendu processeur).

mod common;

use std::time::{Duration, Instant};

use afterdusk_lib::develop::gpu::GpuRenderer;
use afterdusk_lib::develop::pipeline::{Clipping, RenderParams, WorkingImage, render};
use afterdusk_lib::develop::settings::{
    DevelopSettings, LocalAdjust, Mask, MaskShape, WhiteBalance,
};
use afterdusk_lib::develop::source::PREVIEW_SIDE;
use afterdusk_lib::raw::RawFile;

/// Réglages de la photo du test de la V0.2, et huit masques qui se recouvrent.
fn heavy_with_masks() -> DevelopSettings {
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
    let adjust = |k: f64| LocalAdjust {
        exposure: -0.5 * k,
        contrast: 20.0 * k,
        highlights: -30.0,
        shadows: 25.0,
        temperature: 15.0 * k,
        tint: -10.0,
        saturation: 20.0 * k,
    };
    s.masks = (0..8)
        .map(|i| {
            let f = f64::from(i) / 8.0;
            let shape = if i % 2 == 0 {
                MaskShape::Linear {
                    from: [f, 0.0],
                    to: [1.0 - f, 0.6],
                }
            } else {
                MaskShape::Radial {
                    center: [0.2 + 0.6 * f, 0.6],
                    radius: [0.3, 0.2],
                    angle: 20.0 * f64::from(i),
                    feather: 60.0,
                }
            };
            Mask {
                shape,
                adjust: adjust(if i % 3 == 0 { -1.0 } else { 1.0 }),
                invert: i == 5,
                hidden: false,
                strokes: Vec::new(),
            }
        })
        .collect();
    s
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

#[test]
#[ignore = "mesure de performance : cargo test --release --test perf_v06 -- --ignored"]
fn retour_visuel_sous_100_ms_avec_8_masques() {
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
        let mut settings = heavy_with_masks();
        let mut times = Vec::new();
        // L'exposition du troisième masque que l'on fait glisser : 40 valeurs.
        for step in 0..41 {
            settings.masks[2].adjust.exposure = -1.0 + f64::from(step) * 0.05;
            let t = Instant::now();
            // Les masques sont recompilés à chaque réglage, comme dans l'application.
            let p =
                RenderParams::from_settings(&settings, image.as_shot_white, Clipping::default())
                    .with_masks(&settings, image.width.into(), image.height.into());
            let frame = match (&gpu, &uploaded) {
                (Some(g), Some(u)) => g.render(u, &p).unwrap(),
                _ => render(&image, &p),
            };
            let bytes = frame.rgba.to_vec();
            assert_eq!(bytes.len(), (image.width * image.height * 4) as usize);
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
