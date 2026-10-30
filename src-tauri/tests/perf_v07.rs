//! Critère chiffré de la V0.7 : retour visuel d'un réglage en moins de
//! 100 ms avec 8 masques dont des tracés de pinceau longs, mesuré comme ceux
//! de la V0.2 et de la V0.6 sur les vrais RAW de test-assets, tous les
//! réglages de la photo en jeu. On fait glisser l'exposition d'un pinceau ;
//! les grilles de tracés sont refaites à chaque réglage, comme dans
//! l'application. Deux scénarios :
//! - usage courant : deux pinceaux de trois tracés de 300 points, deux
//!   dégradés gommés par un tracé de 200 points, quatre dégradés ;
//! - cas extrême : quatre pinceaux de trois tracés de 1 500 points, quatre
//!   dégradés gommés par deux tracés de 800 points, qui couvrent l'image
//!   plusieurs fois. Au processeur seul, il dépasse le seuil (limite connue,
//!   choix de l'utilisateur du 2026-10-30) : son temps est affiché, le seuil
//!   n'est vérifié qu'avec la carte graphique.
//!
//! `cargo test --release --test perf_v07 -- --ignored --nocapture`
//! (avec `AFTERDUSK_GPU=0` pour mesurer le rendu processeur).

mod common;

use std::time::{Duration, Instant};

use afterdusk_lib::develop::gpu::GpuRenderer;
use afterdusk_lib::develop::pipeline::{Clipping, RenderParams, WorkingImage, render};
use afterdusk_lib::develop::settings::{
    DevelopSettings, LocalAdjust, Mask, MaskShape, Stroke, WhiteBalance,
};
use afterdusk_lib::develop::source::PREVIEW_SIDE;
use afterdusk_lib::raw::RawFile;

/// Réglages de la photo du test de la V0.2.
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

fn adjust(k: f64) -> LocalAdjust {
    LocalAdjust {
        exposure: -0.5 * k,
        contrast: 20.0 * k,
        highlights: -30.0,
        shadows: 25.0,
        temperature: 15.0 * k,
        tint: -10.0,
        saturation: 20.0 * k,
    }
}

/// Dégradé numéro `i` : linéaire ou radial, en alternance.
fn gradient(i: u32) -> MaskShape {
    let f = f64::from(i) / 8.0;
    if i % 2 == 1 {
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
    }
}

fn mask(i: u32, shape: MaskShape, strokes: Vec<Stroke>) -> Mask {
    Mask {
        shape,
        adjust: adjust(if i.is_multiple_of(3) { -1.0 } else { 1.0 }),
        invert: i == 5,
        hidden: false,
        strokes,
    }
}

/// Tracé : `points` points pris le long de `path` (de 0 à 1).
fn stroke(points: usize, erase: bool, path: impl Fn(f64) -> [f64; 2]) -> Stroke {
    Stroke {
        points: (0..points)
            .map(|j| path(j as f64 / points as f64))
            .collect(),
        radius: 0.03,
        feather: 60.0,
        flow: 70.0,
        erase,
    }
}

/// Tracé long et sinueux sur toute l'image (courbe de Lissajous).
fn lissajous(points: usize, phase: f64, erase: bool) -> Stroke {
    stroke(points, erase, |t| {
        let t = t * std::f64::consts::TAU;
        [
            0.5 + 0.4 * (3.0 * t + phase).sin(),
            0.5 + 0.4 * (2.0 * t + phase * 0.7).cos(),
        ]
    })
}

/// Coup de pinceau courant : un quart de la largeur, légèrement ondulé, à
/// partir de `start`.
fn dab(points: usize, start: [f64; 2], phase: f64, erase: bool) -> Stroke {
    stroke(points, erase, |t| {
        [
            start[0] + 0.25 * t,
            start[1] + 0.03 * (6.0 * std::f64::consts::PI * t + phase).sin(),
        ]
    })
}

/// Usage courant : deux pinceaux de trois coups, deux dégradés gommés d'un
/// coup, quatre dégradés.
fn everyday() -> DevelopSettings {
    let mut s = heavy();
    s.masks = (0..8)
        .map(|i| match i {
            0 | 4 => {
                let y = if i == 0 { 0.3 } else { 0.65 };
                let strokes = (0..3)
                    .map(|k| {
                        dab(
                            300,
                            [0.2 + 0.1 * f64::from(k), y + 0.04 * f64::from(k)],
                            f64::from(k),
                            false,
                        )
                    })
                    .collect();
                mask(i, MaskShape::Brush, strokes)
            }
            1 | 2 => mask(
                i,
                gradient(i),
                vec![dab(200, [0.4, 0.2 + 0.3 * f64::from(i)], 0.0, true)],
            ),
            _ => mask(i, gradient(i), Vec::new()),
        })
        .collect();
    s
}

/// Cas extrême : quatre pinceaux de trois longs tracés qui se croisent,
/// quatre dégradés gommés par deux longs tracés.
fn extreme() -> DevelopSettings {
    let mut s = heavy();
    s.masks = (0..8)
        .map(|i| {
            let brush = i % 2 == 0;
            let strokes = (0..if brush { 3 } else { 2 })
                .map(|k| lissajous(if brush { 1500 } else { 800 }, f64::from(i * 3 + k), !brush))
                .collect();
            mask(
                i,
                if brush { MaskShape::Brush } else { gradient(i) },
                strokes,
            )
        })
        .collect();
    s
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// 95e centile du rendu des six RAW avec ces réglages, et le moteur utilisé
/// (vrai : carte graphique).
fn measure(settings: fn() -> DevelopSettings) -> (Duration, bool) {
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
        let mut settings = settings();
        let mut times = Vec::new();
        // L'exposition du premier pinceau que l'on fait glisser : 40 valeurs.
        for step in 0..41 {
            settings.masks[0].adjust.exposure = -1.0 + f64::from(step) * 0.05;
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
    (p95, gpu.is_some())
}

#[test]
#[ignore = "mesure de performance : cargo test --release --test perf_v07 -- --ignored"]
fn usage_courant_sous_100_ms() {
    let (p95, _) = measure(everyday);
    assert!(p95 < Duration::from_millis(100), "{} ms", p95.as_millis());
}

#[test]
#[ignore = "mesure de performance : cargo test --release --test perf_v07 -- --ignored"]
fn cas_extreme_sous_100_ms_sur_la_carte_graphique() {
    let (p95, gpu) = measure(extreme);
    if gpu {
        assert!(p95 < Duration::from_millis(100), "{} ms", p95.as_millis());
    } else {
        println!("processeur seul : limite connue, pas de seuil vérifié");
    }
}
