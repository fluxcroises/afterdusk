//! Prototype LibRaw : aperçu JPEG intégré et EXIF sur les échantillons de test-assets/.

mod common;

use afterdusk_lib::raw::{PreviewFormat, RawFile};

struct Expected {
    file: &'static str,
    make: &'static str,
    xtrans: bool,
    dng: bool,
}

const SAMPLES: &[Expected] = &[
    Expected {
        file: "fujifilm_x-t3.RAF",
        make: "Fujifilm",
        xtrans: true,
        dng: false,
    },
    Expected {
        file: "canon_eos_r6.CR3",
        make: "Canon",
        xtrans: false,
        dng: false,
    },
    Expected {
        file: "nikon_z6.NEF",
        make: "Nikon",
        xtrans: false,
        dng: false,
    },
    Expected {
        file: "sony_ilce-7m3.ARW",
        make: "Sony",
        xtrans: false,
        dng: false,
    },
    Expected {
        file: "panasonic_dc-g9.RW2",
        make: "Panasonic",
        xtrans: false,
        dng: false,
    },
    Expected {
        file: "ricoh_gr-iii.DNG",
        make: "Ricoh",
        xtrans: false,
        dng: true,
    },
];

#[test]
fn apercu_jpeg_et_exif_de_chaque_format() {
    for expected in SAMPLES {
        let path = common::sample(expected.file);
        let mut raw = RawFile::open(&path).unwrap_or_else(|e| panic!("{}: {e}", expected.file));
        let info = raw.info();

        assert!(
            info.make
                .to_lowercase()
                .contains(&expected.make.to_lowercase()),
            "{}: marque {:?}",
            expected.file,
            info.make
        );
        assert!(!info.model.is_empty(), "{}: modèle vide", expected.file);
        assert_eq!(
            info.is_xtrans, expected.xtrans,
            "{}: X-Trans",
            expected.file
        );
        assert_eq!(info.is_dng, expected.dng, "{}: DNG", expected.file);
        assert!(info.iso.is_some(), "{}: ISO absent", expected.file);
        assert!(info.shutter.is_some(), "{}: vitesse absente", expected.file);
        assert!(
            info.captured_at.is_some(),
            "{}: date absente",
            expected.file
        );
        assert!(
            info.width > 0 && info.height > 0,
            "{}: dimensions",
            expected.file
        );

        let preview = raw
            .preview()
            .unwrap_or_else(|e| panic!("{}: {e}", expected.file));
        assert_eq!(preview.format, PreviewFormat::Jpeg, "{}", expected.file);
        assert!(
            preview.data.starts_with(&[0xff, 0xd8, 0xff]),
            "{}: pas un JPEG",
            expected.file
        );
        assert!(
            preview.width.max(preview.height) >= 1024,
            "{}: aperçu trop petit ({}x{}) pour le cache 1024 px",
            expected.file,
            preview.width,
            preview.height
        );
    }
}

/// Règle absolue : aucun original modifié par la lecture.
#[test]
fn la_lecture_ne_modifie_pas_les_originaux() {
    for expected in SAMPLES {
        let path = common::sample(expected.file);
        let before = common::sha256(&path);
        let modified = common::modified(&path);
        {
            let mut raw = RawFile::open(&path).unwrap();
            let _ = raw.info();
            let _ = raw.exif();
            let _ = raw.preview().unwrap();
        }
        assert_eq!(common::sha256(&path), before, "{} modifié", expected.file);
        assert_eq!(
            common::modified(&path),
            modified,
            "{} touché",
            expected.file
        );
    }
}

#[test]
fn fichier_inexistant_renvoie_une_erreur() {
    let path = common::test_assets().join("absent.RAF");
    assert!(RawFile::open(&path).is_err());
}

#[test]
fn chemin_unicode() {
    let source = common::sample("nikon_z6.NEF");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Été à Saint-Malo – 日本.NEF");
    std::fs::copy(&source, &path).unwrap();
    let mut raw = RawFile::open(&path).unwrap();
    assert!(raw.preview().is_ok());
}
