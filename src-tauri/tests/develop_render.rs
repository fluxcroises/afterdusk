//! Rendu neutre (processus 1) sur les six formats de test-assets, comparé au
//! JPEG intégré du boîtier ; les originaux ne sont jamais modifiés.

mod common;

use std::time::Instant;

use afterdusk_lib::develop::pipeline::{WorkingImage, render_rgba};
use afterdusk_lib::develop::settings::DevelopSettings;
use afterdusk_lib::develop::white;
use afterdusk_lib::raw::RawFile;
use afterdusk_lib::raw::preview::{decode_jpeg_scaled, orient};

/// Luminance moyenne (codée sRGB, 0 à 1) d'une image RGBA ou RGB.
fn mean_luma(pixels: &[u8], stride: usize) -> f64 {
    let n = pixels.len() / stride;
    let sum: f64 = pixels
        .chunks_exact(stride)
        .map(|p| 0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]))
        .sum();
    sum / (n as f64 * 255.0)
}

#[test]
fn rendu_neutre_proche_du_jpeg_du_boitier() {
    let mut report = Vec::new();
    for name in common::RAW_SAMPLES {
        let path = common::sample(name);
        let before = common::sha256(&path);
        let mut raw = RawFile::open(&path).unwrap();
        let orientation = raw.info().orientation;
        let t0 = Instant::now();
        let developed = raw.develop(true).unwrap();
        let decode = t0.elapsed();
        // Balance du boîtier lue dans ses multiplicateurs : une lumière plausible.
        let (temperature, tint) = white::temperature_of(white::xy_of(developed.as_shot_white));
        assert!(
            (2500.0..10_000.0).contains(&temperature) && tint.abs() < 60.0,
            "{name} : {temperature:.0} K, teinte {tint:.0}"
        );
        let working = WorkingImage::from_raw(&developed, 1024);
        let t1 = Instant::now();
        let rgba = render_rgba(&working, &DevelopSettings::default());
        let render = t1.elapsed();
        assert_eq!(rgba.len(), (working.width * working.height * 4) as usize);

        // Le JPEG intégré, orienté comme notre rendu, à taille voisine.
        let preview = raw.preview().unwrap();
        let jpeg = orient(
            decode_jpeg_scaled(&preview.data, 1024).unwrap(),
            orientation,
        );
        let ours = mean_luma(&rgba, 4);
        let theirs = mean_luma(jpeg.as_raw(), 3);
        // Même orientation : rapport largeur/hauteur voisin.
        let ratio_ours = f64::from(working.width) / f64::from(working.height);
        let ratio_jpeg = f64::from(jpeg.width()) / f64::from(jpeg.height());
        assert!(
            (ratio_ours > 1.0) == (ratio_jpeg > 1.0),
            "{name} : orientation {ratio_ours:.2} contre {ratio_jpeg:.2}"
        );
        report.push(format!(
            "{name:22} {}×{} décodage {:>4} ms, rendu {:>3} ms, luminance {ours:.3} (JPEG {theirs:.3}, rapport {:.2}), boîtier {temperature:.0} K / {tint:+.0}",
            working.width,
            working.height,
            decode.as_millis(),
            render.as_millis(),
            ours / theirs
        ));
        // Neutre mais pas méconnaissable : à moins d'un quart du JPEG du boîtier.
        assert!(
            (ours / theirs - 1.0).abs() < 0.25,
            "{name} : luminance {ours:.3} contre {theirs:.3}"
        );
        assert_eq!(common::sha256(&path), before, "{name} modifié");
    }
    println!("{}", report.join("\n"));
}

#[test]
fn auto_sur_les_six_formats() {
    use afterdusk_lib::develop::auto::{auto_light, auto_white};
    use afterdusk_lib::develop::pipeline::{Clipping, RenderParams, render};
    use afterdusk_lib::develop::settings::WhiteBalance;

    for name in common::RAW_SAMPLES {
        let mut raw = RawFile::open(&common::sample(name)).unwrap();
        let image = WorkingImage::from_raw(&raw.develop(true).unwrap(), 2048);
        let t = Instant::now();
        let mut s = DevelopSettings::default();
        s.light = auto_light(&image, &s);
        let light_ms = t.elapsed().as_millis();
        let t = Instant::now();
        let wb = auto_white(&image);
        let white_ms = t.elapsed().as_millis();

        // Rendu final : médiane près du gris moyen, presque rien de brûlé.
        let p = RenderParams::from_settings(&s, image.as_shot_white, Clipping::default());
        let h = render(&image, &p).histogram;
        let luma = &h[768..1024];
        let total: u32 = luma.iter().sum();
        let mut acc = 0;
        let median = luma
            .iter()
            .position(|n| {
                acc += n;
                acc * 2 >= total
            })
            .unwrap_or(255);
        let burnt = f64::from(luma[250..].iter().sum::<u32>()) / f64::from(total);
        let (temperature, tint) = match wb {
            WhiteBalance::Custom { temperature, tint } => (temperature, tint),
            _ => (0.0, 0.0),
        };
        println!(
            "{name:22} {light_ms:>3} ms : expo {:+.2}, HL {}, omb {}, bl {}, n {}, contr {} → médiane {median}, brûlé {:.2} % ; balance {white_ms} ms : {temperature} K / {tint:+}",
            s.light.exposure,
            s.light.highlights,
            s.light.shadows,
            s.light.whites,
            s.light.blacks,
            s.light.contrast,
            burnt * 100.0
        );
        // Le feuillage du Panasonic, très contrasté (ciel entre les feuilles),
        // reste plus sombre : les hautes lumières passent avant la médiane.
        assert!((75..=130).contains(&median), "{name} : médiane {median}");
        assert!(burnt < 0.03, "{name} : {burnt}");
        assert!(
            (2500.0..12_000.0).contains(&temperature) && tint.abs() <= 35.0,
            "{name} : {wb:?}"
        );
    }
}
