//! Rendu neutre (processus 1) sur les six formats de test-assets, comparé au
//! JPEG intégré du boîtier ; les originaux ne sont jamais modifiés.

mod common;

use std::time::Instant;

use afterdusk_lib::develop::pipeline::{WorkingImage, render_rgba};
use afterdusk_lib::develop::settings::DevelopSettings;
use afterdusk_lib::raw::RawFile;
use afterdusk_lib::raw::preview::{decode_jpeg_scaled, orient};

const SAMPLES: [&str; 6] = [
    "fujifilm_x-t3.RAF",
    "canon_eos_r6.CR3",
    "nikon_z6.NEF",
    "sony_ilce-7m3.ARW",
    "panasonic_dc-g9.RW2",
    "ricoh_gr-iii.DNG",
];

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
    for name in SAMPLES {
        let path = common::sample(name);
        let before = common::sha256(&path);
        let mut raw = RawFile::open(&path).unwrap();
        let orientation = raw.info().orientation;
        let t0 = Instant::now();
        let developed = raw.develop(true).unwrap();
        let decode = t0.elapsed();
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
            "{name:22} {}×{} décodage {:>4} ms, rendu {:>3} ms, luminance {ours:.3} (JPEG {theirs:.3}, rapport {:.2})",
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
