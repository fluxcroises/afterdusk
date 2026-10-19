//! Le jumeau GPU rend comme la référence processeur, à un niveau près.
//! Sans processeur graphique utilisable (machine d'intégration), le test le
//! signale et s'arrête : l'application revient alors au rendu processeur.

mod common;

use std::time::Instant;

use afterdusk_lib::develop::gpu::GpuRenderer;
use afterdusk_lib::develop::pipeline::{WorkingImage, render_rgba};
use afterdusk_lib::develop::settings::DevelopSettings;
use afterdusk_lib::raw::RawFile;

/// Écart maximal toléré entre les deux rendus, en niveaux sur 255.
const TOLERANCE: u8 = 1;

fn max_diff(a: &[u8], b: &[u8]) -> (u8, usize) {
    assert_eq!(a.len(), b.len());
    let diffs = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y));
    let over = a.iter().zip(b).filter(|(x, y)| x.abs_diff(**y) > 0).count();
    (diffs.max().unwrap_or(0), over)
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
    };
    let uploaded = gpu.upload(&image).unwrap();
    for exposure in [-2.0, 0.0, 1.5] {
        let mut s = DevelopSettings::default();
        s.light.exposure = exposure;
        let cpu = render_rgba(&image, &s);
        let ours = gpu.render(&uploaded, &s).unwrap();
        let (max, n) = max_diff(&cpu, &ours);
        println!("exposition {exposure:+} : écart max {max}, {n} valeurs différentes");
        assert!(max <= TOLERANCE, "écart {max} à {exposure:+} IL");
    }
}

#[test]
fn six_formats_identiques_et_temps() {
    let Some(gpu) = renderer() else { return };
    for name in common::RAW_SAMPLES {
        let mut raw = RawFile::open(&common::sample(name)).unwrap();
        let image = WorkingImage::from_raw(&raw.develop(true).unwrap(), 2048);
        let s = DevelopSettings::default();
        let t = Instant::now();
        let cpu = render_rgba(&image, &s);
        let cpu_time = t.elapsed();
        let uploaded = gpu.upload(&image).unwrap();
        gpu.render(&uploaded, &s).unwrap(); // première passe : préparation
        let t = Instant::now();
        let ours = gpu.render(&uploaded, &s).unwrap();
        let gpu_time = t.elapsed();
        let (max, n) = max_diff(&cpu, &ours);
        println!(
            "{name:22} {}×{} processeur {:>3} ms, GPU {:>3} ms, écart max {max} ({n} valeurs)",
            image.width,
            image.height,
            cpu_time.as_millis(),
            gpu_time.as_millis()
        );
        assert!(max <= TOLERANCE, "{name} : écart {max}");
    }
}
