//! Aperçus en cache à partir des vrais RAW de test-assets/.

mod common;

use std::time::Instant;

use afterdusk_lib::metadata::media::MediaFormat;
use afterdusk_lib::raw::preview::{self, SIZES};

const SAMPLES: &[(&str, u32)] = &[
    // (fichier, côté long de l'aperçu intégré)
    ("fujifilm_x-t3.RAF", 4416),
    ("canon_eos_r6.CR3", 5472),
    ("nikon_z6.NEF", 6048),
    ("sony_ilce-7m3.ARW", 1616),
    ("panasonic_dc-g9.RW2", 1920),
    ("ricoh_gr-iii.DNG", 6000),
];

#[test]
fn trois_paliers_pour_chaque_format() {
    let cache = tempfile::tempdir().unwrap();
    for (index, (file, embedded)) in SAMPLES.iter().enumerate() {
        let path = common::sample(file);
        let before = common::sha256(&path);
        let start = Instant::now();
        let out = preview::generate(&path, MediaFormat::Raw, 1, cache.path(), index as i64)
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        eprintln!("{file}: {} ms", start.elapsed().as_millis());

        for (generated, &size) in out.iter().zip(SIZES.iter()) {
            let long = generated.width.max(generated.height);
            assert_eq!(long, size.min(*embedded), "{file} palier {size}");
            let path = preview::cache_path(cache.path(), index as i64, size);
            assert!(std::fs::read(&path).unwrap().starts_with(&[0xff, 0xd8]));
        }
        // Règle absolue : générer un aperçu ne touche pas l'original.
        assert_eq!(common::sha256(&path), before, "{file}");
    }
}

#[test]
fn miniature_de_la_source() {
    let path = common::sample("nikon_z6.NEF");
    let jpeg = preview::thumbnail(&path, MediaFormat::Raw, 1, 256).unwrap();
    let img = image::load_from_memory(&jpeg).unwrap();
    assert_eq!(img.width().max(img.height()), 256);
}
