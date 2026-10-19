//! Le jumeau GPU rend comme la référence processeur, à un niveau près, avec le
//! même histogramme à ces écarts près. Sans processeur graphique utilisable
//! (machine d'intégration), le test le signale et s'arrête : l'application
//! revient alors au rendu processeur.

mod common;

use std::time::Instant;

use afterdusk_lib::develop::gpu::GpuRenderer;
use afterdusk_lib::develop::pipeline::{Clipping, RenderParams, WorkingImage, render};
use afterdusk_lib::develop::settings::{DevelopSettings, WhiteBalance};
use afterdusk_lib::raw::{D65_WHITE, RawFile};

/// Écart maximal toléré entre les deux rendus, en niveaux sur 255.
const TOLERANCE: u8 = 1;

/// Écart maximal et nombre de valeurs différentes.
fn max_diff(a: &[u8], b: &[u8]) -> (u8, usize) {
    assert_eq!(a.len(), b.len());
    let max = a
        .iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0);
    let over = a.iter().zip(b).filter(|(x, y)| x != y).count();
    (max, over)
}

/// Un pixel qui change d'un niveau déplace un compte d'une case à sa voisine
/// dans son canal et, au plus, dans la luminance : quatre comptes d'écart.
fn histograms_agree(a: &[u32], b: &[u32], differing_values: usize) -> bool {
    let l1: u64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    l1 <= 4 * differing_values as u64
}

fn renderer() -> Option<GpuRenderer> {
    match GpuRenderer::new() {
        Ok(r) => {
            println!("processeur graphique : {}", r.adapter_name());
            Some(r)
        }
        Err(e) => {
            println!("test GPU sauté : {e}");
            None
        }
    }
}

/// Jeux de réglages qui exercent chaque étape du pipeline.
fn cases() -> Vec<(&'static str, DevelopSettings, Clipping)> {
    let mut all = DevelopSettings::default();
    all.light.exposure = 0.6;
    all.light.brightness = 35.0;
    all.light.contrast = 40.0;
    all.light.highlights = -70.0;
    all.light.shadows = 60.0;
    all.light.whites = 20.0;
    all.light.blacks = -25.0;
    all.color.vibrance = 45.0;
    all.color.saturation = -20.0;
    all.color.white_balance = WhiteBalance::Custom {
        temperature: 4200.0,
        tint: 12.0,
    };
    let mut dark = DevelopSettings::default();
    dark.light.exposure = -2.0;
    dark.light.contrast = -60.0;
    dark.color.saturation = 80.0;
    dark.color.white_balance = WhiteBalance::Preset {
        preset: "tungsten".into(),
    };
    let both = Clipping {
        highlights: true,
        shadows: true,
    };
    vec![
        ("neutre", DevelopSettings::default(), Clipping::default()),
        ("tous les curseurs", all, both),
        ("sombre et tungstène", dark, Clipping::default()),
    ]
}

#[test]
fn degrade_synthetique_identique() {
    let Some(gpu) = renderer() else { return };
    // Toutes les teintes et intensités, des ombres aux hautes lumières écrêtées.
    let (w, h) = (512u32, 300u32);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = 2.0 * x as f32 / w as f32;
            let t = y as f32 / h as f32;
            rgb.extend_from_slice(&[v * t, v * (1.0 - t), v * 0.5 - 0.05]);
        }
    }
    let image = WorkingImage {
        width: w,
        height: h,
        rgb,
        as_shot_white: D65_WHITE,
    };
    let uploaded = gpu.upload(&image).unwrap();
    for (name, settings, clipping) in cases() {
        let p = RenderParams::from_settings(&settings, image.as_shot_white, clipping);
        let cpu = render(&image, &p);
        let ours = gpu.render(&uploaded, &p).unwrap();
        let (max, n) = max_diff(&cpu.rgba, &ours.rgba);
        println!("{name} : écart max {max}, {n} valeurs différentes");
        assert!(max <= TOLERANCE, "{name} : écart {max}");
        assert!(
            histograms_agree(&cpu.histogram, &ours.histogram, n),
            "{name} : histogrammes"
        );
    }
}

#[test]
fn six_formats_identiques_et_temps() {
    let Some(gpu) = renderer() else { return };
    for name in common::RAW_SAMPLES {
        let mut raw = RawFile::open(&common::sample(name)).unwrap();
        let image = WorkingImage::from_raw(&raw.develop(true).unwrap(), 2048);
        let uploaded = gpu.upload(&image).unwrap();
        for (case, settings, clipping) in cases() {
            let p = RenderParams::from_settings(&settings, image.as_shot_white, clipping);
            let t = Instant::now();
            let cpu = render(&image, &p);
            let cpu_time = t.elapsed();
            gpu.render(&uploaded, &p).unwrap(); // première passe : préparation
            let t = Instant::now();
            let ours = gpu.render(&uploaded, &p).unwrap();
            let gpu_time = t.elapsed();
            let (max, n) = max_diff(&cpu.rgba, &ours.rgba);
            println!(
                "{name:22} {case:20} {}×{} processeur {:>3} ms, GPU {:>3} ms, écart max {max} ({n} valeurs)",
                image.width,
                image.height,
                cpu_time.as_millis(),
                gpu_time.as_millis()
            );
            assert!(max <= TOLERANCE, "{name} {case} : écart {max}");
            assert!(
                histograms_agree(&cpu.histogram, &ours.histogram, n),
                "{name} {case} : histogrammes"
            );
        }
    }
}
