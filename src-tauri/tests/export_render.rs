//! Export sur les échantillons : rendu, taille, formats, durée.

use std::path::Path;
use std::time::Instant;

use afterdusk_lib::develop::lens::{self, Shot};
use afterdusk_lib::develop::settings::DevelopSettings;
use afterdusk_lib::export::color::ColorSpace;
use afterdusk_lib::export::encode::{self, Embedded, Format};
use afterdusk_lib::export::render::{RenderOptions, Resize, Sharpen, render_photo};
use afterdusk_lib::raw::RawFile;

const FILES: [&str; 6] = [
    "canon_eos_r6.CR3",
    "fujifilm_x-t3.RAF",
    "nikon_z6.NEF",
    "panasonic_dc-g9.RW2",
    "ricoh_gr-iii.DNG",
    "sony_ilce-7m3.ARW",
];

/// JPEG 2048 px des six échantillons, avec une retouche et un recadrage :
/// taille exacte, fichier relisible, durée par photo (objectif : 100 photos
/// en moins de 2 minutes, soit 1,2 s par photo).
#[test]
fn jpeg_web_des_six_echantillons() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-assets");
    let out = tempfile::tempdir().unwrap();
    let mut settings = DevelopSettings::default();
    settings.light.exposure = 0.3;
    settings.light.contrast = 20.0;
    settings.color.vibrance = 15.0;
    settings.geometry.angle = 1.5;
    settings.geometry.crop.left = 0.1;
    settings.geometry.crop.right = 0.9;
    settings.geometry = {
        let mut g = settings.geometry.clone();
        g.crop = afterdusk_lib::develop::geometry::constrain(&g, 3000.0, 2000.0);
        g
    };
    let options = RenderOptions {
        resize: Resize::LongSide(2048),
        sharpen: Sharpen::Screen,
        color: ColorSpace::Srgb,
    };
    let icc = ColorSpace::Srgb.icc().unwrap();
    let mut times = Vec::new();
    for file in FILES {
        let started = Instant::now();
        let mut raw = RawFile::open(&dir.join(file)).unwrap();
        let profile = lens::find_profile(&Shot::from(&raw.info()));
        let img = render_photo(&mut raw, &settings, profile.as_ref(), options, None).unwrap();
        assert_eq!(img.width.max(img.height), 2048, "{file}");
        let path = out.path().join(format!("{file}.jpg"));
        encode::write(
            &img,
            Format::Jpeg { quality: 85 },
            &icc,
            &Embedded::default(),
            &path,
        )
        .unwrap();
        let ms = started.elapsed().as_millis();
        let decoded = image::open(&path).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (img.width, img.height));
        times.push(format!("{file} {ms} ms"));
    }
    eprintln!("{}", times.join(" · "));
}

/// Pleine résolution en TIFF 16 bits Adobe RGB : la taille de l'original.
#[test]
fn tiff_16_bits_pleine_resolution() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-assets");
    let out = tempfile::tempdir().unwrap();
    let mut raw = RawFile::open(&dir.join("nikon_z6.NEF")).unwrap();
    let info = raw.info();
    let options = RenderOptions {
        resize: Resize::Full,
        sharpen: Sharpen::Paper,
        color: ColorSpace::AdobeRgb,
    };
    let started = Instant::now();
    let img = render_photo(&mut raw, &DevelopSettings::default(), None, options, None).unwrap();
    let long = img.width.max(img.height);
    assert!(
        long + 64 >= u32::from(info.width.max(info.height)),
        "{long}"
    );
    let path = out.path().join("z6.tif");
    let icc = ColorSpace::AdobeRgb.icc().unwrap();
    encode::write(
        &img,
        Format::Tiff { sixteen: true },
        &icc,
        &Embedded::default(),
        &path,
    )
    .unwrap();
    eprintln!(
        "TIFF 16 bits {} × {} en {} ms, {} Mo",
        img.width,
        img.height,
        started.elapsed().as_millis(),
        std::fs::metadata(&path).unwrap().len() / 1_000_000
    );
    let decoded = image::open(&path).unwrap();
    assert_eq!(decoded.color(), image::ColorType::Rgb16);
}

/// Filigrane texte et métadonnées sur une vraie photo : le JPEG garde son
/// EXIF (sans GPS ni numéro de série) ; copie pour contrôle à l'œil dans
/// `AFTERDUSK_EXPORT_SAMPLE` si la variable est posée.
#[test]
fn filigrane_et_metadonnees() {
    use afterdusk_lib::export::metadata::{self, MetadataPolicy, PhotoMeta};
    use afterdusk_lib::export::watermark::{Anchor, Watermark, WatermarkContent};
    use afterdusk_lib::metadata::media::{MediaFormat, read_metadata};
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-assets");
    let src = dir.join("sony_ilce-7m3.ARW");
    let out = tempfile::tempdir().unwrap();
    let mut raw = RawFile::open(&src).unwrap();
    let mark = Watermark {
        content: WatermarkContent::Text {
            text: "© Lewis Bedar".into(),
            dark: false,
        },
        anchor: Anchor::BottomRight,
        size: 25.0,
        opacity: 70.0,
        margin: 3.0,
    };
    let options = RenderOptions {
        resize: Resize::LongSide(1600),
        sharpen: Sharpen::Screen,
        color: ColorSpace::Srgb,
    };
    let img = render_photo(
        &mut raw,
        &DevelopSettings::default(),
        None,
        options,
        Some(&mark),
    )
    .unwrap();
    let meta = PhotoMeta {
        camera: read_metadata(&src, MediaFormat::Raw),
        sidecar: None,
    };
    let embedded = metadata::embedded(&meta, MetadataPolicy::NoGpsSerial).unwrap();
    let path = out.path().join("web.jpg");
    let icc = ColorSpace::Srgb.icc().unwrap();
    encode::write(&img, Format::Jpeg { quality: 85 }, &icc, &embedded, &path).unwrap();
    let file = std::fs::File::open(&path).unwrap();
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(file))
        .unwrap();
    use exif::{In, Tag};
    assert!(exif.get_field(Tag::Model, In::PRIMARY).is_some());
    assert!(exif.get_field(Tag::BodySerialNumber, In::PRIMARY).is_none());
    assert!(exif.get_field(Tag::GPSLatitude, In::PRIMARY).is_none());
    if let Some(dest) = std::env::var_os("AFTERDUSK_EXPORT_SAMPLE") {
        std::fs::copy(&path, dest).unwrap();
    }
}
