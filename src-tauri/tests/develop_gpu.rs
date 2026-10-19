//! Le jumeau GPU rend comme la référence processeur, à un niveau près, avec le
//! même histogramme à ces écarts près. Sans processeur graphique utilisable
//! (machine d'intégration), le test le signale et s'arrête : l'application
//! revient alors au rendu processeur.

mod common;

use std::time::Instant;

use afterdusk_lib::develop::gpu::{GpuError, GpuRenderer};
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

/// Avec les avertissements d'écrêtage, un pixel à 254 d'un côté et 255 de
/// l'autre n'est peint que d'un côté : seuls les pixels qui différaient déjà
/// sans avertissements peuvent différer.
fn overlay_agrees(cpu: &[u8], gpu: &[u8], clean_cpu: &[u8], clean_gpu: &[u8]) -> bool {
    let px = |a: &[u8]| a.as_chunks::<4>().0.to_vec();
    let (c, g, cc, cg) = (px(cpu), px(gpu), px(clean_cpu), px(clean_gpu));
    c.iter()
        .zip(&g)
        .zip(cc.iter().zip(&cg))
        .all(|((a, b), (ca, cb))| a == b || ca != cb)
}

/// Compare un jeu de réglages : sans avertissements à un niveau près, avec
/// histogrammes ; avec avertissements, aux pixels limites près.
fn compare(
    label: &str,
    gpu: &GpuRenderer,
    uploaded: &afterdusk_lib::develop::gpu::GpuImage,
    image: &WorkingImage,
    settings: &DevelopSettings,
    clipping: Clipping,
) -> usize {
    let clean = RenderParams::from_settings(settings, image.as_shot_white, Clipping::default());
    let cpu = render(image, &clean);
    let ours = gpu.render(uploaded, &clean).unwrap();
    let (max, n) = max_diff(&cpu.rgba, &ours.rgba);
    assert!(max <= TOLERANCE, "{label} : écart {max}");
    assert!(
        histograms_agree(&cpu.histogram, &ours.histogram, n),
        "{label} : histogrammes"
    );
    if clipping != Clipping::default() {
        let p = RenderParams::from_settings(settings, image.as_shot_white, clipping);
        let cpu_w = render(image, &p);
        let ours_w = gpu.render(uploaded, &p).unwrap();
        assert!(
            overlay_agrees(&cpu_w.rgba, &ours_w.rgba, &cpu.rgba, &ours.rgba),
            "{label} : avertissements d'écrêtage"
        );
    }
    n
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

/// Le processeur graphique, ou rien s'il n'y en a pas (le test est alors
/// sauté). Toute autre erreur (shader refusé, disposition invalide) échoue :
/// elle ne doit jamais passer pour une machine sans GPU.
fn renderer() -> Option<GpuRenderer> {
    match GpuRenderer::new() {
        Ok(r) => {
            println!("processeur graphique : {}", r.adapter_name());
            Some(r)
        }
        Err(e @ (GpuError::NoAdapter(_) | GpuError::Device(_))) => {
            println!("test GPU sauté : {e}");
            None
        }
        Err(e) => panic!("processeur graphique présent mais inutilisable : {e}"),
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
    let mut look = DevelopSettings::default();
    look.curve.rgb = vec![[0.0, 18.0], [70.0, 55.0], [190.0, 205.0], [255.0, 245.0]];
    look.curve.blue = vec![[0.0, 30.0], [255.0, 230.0]];
    look.hsl.hue.orange = -40.0;
    look.hsl.saturation.blue = 60.0;
    look.hsl.saturation.green = -70.0;
    look.hsl.luminance.aqua = -50.0;
    look.hsl.luminance.red = 35.0;
    look.effects.vignette.amount = -45.0;
    look.effects.vignette.midpoint = 30.0;
    look.effects.grain.amount = 60.0;
    look.effects.grain.size = 40.0;
    look.effects.grain.roughness = 70.0;
    let mut detail = DevelopSettings::default();
    detail.light.exposure = 1.2;
    detail.detail.noise.luminance = 70.0;
    detail.detail.noise.color = 60.0;
    detail.detail.sharpening.amount = 90.0;
    detail.detail.sharpening.radius = 1.4;
    detail.detail.sharpening.masking = 30.0;
    detail.effects.grain.amount = 30.0;
    let mut sharp_only = DevelopSettings::default();
    sharp_only.detail.sharpening.amount = 150.0;
    sharp_only.detail.sharpening.detail = 100.0;
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
        ("courbes, TSL, effets", look, Clipping::default()),
        ("bruit et netteté", detail, Clipping::default()),
        ("netteté seule", sharp_only, Clipping::default()),
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
        scale: 1.0,
        crop: None,
    };
    let uploaded = gpu.upload(&image).unwrap();
    for (name, settings, clipping) in cases() {
        let n = compare(name, &gpu, &uploaded, &image, &settings, clipping);
        println!("{name} : {n} valeurs différentes d'un niveau");
    }
}

#[test]
fn region_au_zoom_identique_au_processeur() {
    let Some(gpu) = renderer() else { return };
    let mut raw = RawFile::open(&common::sample("fujifilm_x-t3.RAF")).unwrap();
    let full = raw.develop(false).unwrap();
    let part = WorkingImage::crop_from_raw(&full, 3000, 1500, 900, 600);
    let uploaded = gpu.upload(&part).unwrap();
    for (case, settings, clipping) in cases() {
        let n = compare(
            &format!("région {case}"),
            &gpu,
            &uploaded,
            &part,
            &settings,
            clipping,
        );
        println!("région {case} : {n} valeurs différentes d'un niveau");
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
            render(&image, &p);
            let cpu_time = t.elapsed();
            gpu.render(&uploaded, &p).unwrap(); // première passe : préparation
            let t = Instant::now();
            gpu.render(&uploaded, &p).unwrap();
            let gpu_time = t.elapsed();
            let n = compare(
                &format!("{name} {case}"),
                &gpu,
                &uploaded,
                &image,
                &settings,
                clipping,
            );
            println!(
                "{name:22} {case:20} {}×{} processeur {:>3} ms, GPU {:>3} ms, {n} valeurs différentes d'un niveau",
                image.width,
                image.height,
                cpu_time.as_millis(),
                gpu_time.as_millis()
            );
        }
    }
}
