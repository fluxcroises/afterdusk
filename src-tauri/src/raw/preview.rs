//! Aperçus : décodage réduit du JPEG intégré aux RAW (ou de l'image elle-même),
//! orientation appliquée, paliers 256, 1024 et 2048 px encodés en JPEG.
//!
//! Le JPEG est décodé directement à taille réduite (mise à l'échelle lors de
//! la transformée inverse) : un aperçu de 6000 px n'est jamais décodé en entier
//! pour produire une miniature.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, RgbImage};
use jpeg_decoder::{Decoder, PixelFormat};

use super::{PreviewFormat, RawFile};
use crate::metadata::media::MediaFormat;

/// Paliers du cache (cahier des charges, section 3).
pub const SIZES: [u32; 3] = [256, 1024, 2048];
const JPEG_QUALITY: u8 = 85;

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error(transparent)]
    Raw(#[from] super::RawError),
    #[error("décodage JPEG impossible : {0}")]
    Jpeg(#[from] jpeg_decoder::Error),
    #[error("image illisible : {0}")]
    Image(#[from] image::ImageError),
    #[error("format sans aperçu pris en charge : {0}")]
    Unsupported(PathBuf),
    #[error("données d'aperçu incohérentes")]
    Corrupt,
    #[error("écriture impossible de {path} : {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Décode un JPEG en visant un côté long d'au moins `min_side` pixels.
pub fn decode_jpeg_scaled(data: &[u8], min_side: u32) -> Result<RgbImage, PreviewError> {
    let mut decoder = Decoder::new(Cursor::new(data));
    decoder.read_info()?;
    let info = decoder.info().ok_or(PreviewError::Corrupt)?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    if w == 0 || h == 0 {
        return Err(PreviewError::Corrupt);
    }
    let long = w.max(h);
    if long > min_side {
        let ratio = f64::from(min_side) / f64::from(long);
        let req = |v: u32| {
            (f64::from(v) * ratio)
                .ceil()
                .clamp(1.0, f64::from(u16::MAX)) as u16
        };
        decoder.scale(req(w), req(h))?;
    }
    let pixels = decoder.decode()?;
    let info = decoder.info().ok_or(PreviewError::Corrupt)?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    let rgb = match info.pixel_format {
        PixelFormat::RGB24 => pixels,
        PixelFormat::L8 => pixels.iter().flat_map(|&l| [l, l, l]).collect(),
        // Octet de poids fort en premier.
        PixelFormat::L16 => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[hi, _]| [hi, hi, hi])
            .collect(),
        PixelFormat::CMYK32 => pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[c, m, y, k]| {
                let k = u16::from(k);
                let ch = |v: u8| ((u16::from(v) * k) / 255) as u8;
                [ch(c), ch(m), ch(y)]
            })
            .collect(),
    };
    RgbImage::from_raw(w, h, rgb).ok_or(PreviewError::Corrupt)
}

/// Applique une orientation EXIF (1 à 8).
pub fn orient(img: RgbImage, orientation: u8) -> RgbImage {
    let img = DynamicImage::ImageRgb8(img);
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
    .into_rgb8()
}

/// Réduit l'image pour que son côté long ne dépasse pas `max_side`.
pub fn fit(img: &RgbImage, max_side: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    if w.max(h) <= max_side {
        return img.clone();
    }
    let ratio = f64::from(max_side) / f64::from(w.max(h));
    let nw = ((f64::from(w) * ratio).round() as u32).max(1);
    let nh = ((f64::from(h) * ratio).round() as u32).max(1);
    image::imageops::resize(img, nw, nh, FilterType::Triangle)
}

pub fn encode_jpeg(img: &RgbImage) -> Result<Vec<u8>, PreviewError> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(img)?;
    Ok(out)
}

/// Image source de l'aperçu, décodée au plus près de `min_side`, orientée.
pub fn load(
    path: &Path,
    format: MediaFormat,
    orientation: u8,
    min_side: u32,
) -> Result<RgbImage, PreviewError> {
    let img = match format {
        MediaFormat::Raw => {
            let mut raw = RawFile::open(path)?;
            let preview = raw.preview_for(min_side)?;
            match preview.format {
                PreviewFormat::Jpeg => decode_jpeg_scaled(&preview.data, min_side)?,
                PreviewFormat::Bitmap => RgbImage::from_raw(
                    u32::from(preview.width),
                    u32::from(preview.height),
                    preview.data,
                )
                .ok_or(PreviewError::Corrupt)?,
                PreviewFormat::JpegXl => return Err(PreviewError::Unsupported(path.to_path_buf())),
            }
        }
        MediaFormat::Image => {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            match ext.as_str() {
                "jpg" | "jpeg" => {
                    let data = std::fs::read(path).map_err(|source| PreviewError::Io {
                        path: path.to_path_buf(),
                        source,
                    })?;
                    decode_jpeg_scaled(&data, min_side)?
                }
                "png" | "tif" | "tiff" => image::open(path)?.into_rgb8(),
                // HEIF : pas de décodeur compatible GPL v3 intégré pour l'instant.
                _ => return Err(PreviewError::Unsupported(path.to_path_buf())),
            }
        }
    };
    Ok(orient(img, orientation))
}

/// Miniature JPEG d'un fichier (grille d'import, avant toute copie).
pub fn thumbnail(
    path: &Path,
    format: MediaFormat,
    orientation: u8,
    max_side: u32,
) -> Result<Vec<u8>, PreviewError> {
    let img = load(path, format, orientation, max_side)?;
    encode_jpeg(&fit(&img, max_side))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

/// Fichier du cache pour un média et un palier.
pub fn cache_path(cache_root: &Path, media_id: i64, size: u32) -> PathBuf {
    cache_root
        .join(format!("{:04}", media_id / 1000))
        .join(format!("{media_id}-{size}.jpg"))
}

/// Produit les trois paliers en une seule lecture du fichier. Un palier plus
/// grand que l'aperçu disponible reçoit l'aperçu à sa taille d'origine.
pub fn generate(
    path: &Path,
    format: MediaFormat,
    orientation: u8,
    cache_root: &Path,
    media_id: i64,
) -> Result<Vec<Generated>, PreviewError> {
    let largest = SIZES[SIZES.len() - 1];
    let mut current = load(path, format, orientation, largest)?;
    let mut out = Vec::new();
    // Du plus grand au plus petit : chaque palier part du précédent.
    for &size in SIZES.iter().rev() {
        current = fit(&current, size);
        let data = encode_jpeg(&current)?;
        let target = cache_path(cache_root, media_id, size);
        write_atomically(&target, &data)?;
        out.push(Generated {
            size,
            width: current.width(),
            height: current.height(),
            bytes: data.len() as u64,
        });
    }
    out.reverse();
    Ok(out)
}

fn write_atomically(target: &Path, data: &[u8]) -> Result<(), PreviewError> {
    let io = |source| PreviewError::Io {
        path: target.to_path_buf(),
        source,
    };
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(io)?;
    }
    let tmp = target.with_extension("jpg.tmp");
    std::fs::write(&tmp, data).map_err(io)?;
    std::fs::rename(&tmp, target).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        })
    }

    #[test]
    fn decodage_reduit_garde_au_moins_la_taille_demandee() {
        let jpeg = encode_jpeg(&gradient(4000, 3000)).unwrap();
        let img = decode_jpeg_scaled(&jpeg, 1024).unwrap();
        assert!(img.width() >= 1024 && img.width() < 4000, "{}", img.width());
        assert_eq!(img.width() * 3, img.height() * 4);
    }

    #[test]
    fn orientation_portrait() {
        let img = orient(gradient(30, 20), 6);
        assert_eq!(img.dimensions(), (20, 30));
        let img = orient(gradient(30, 20), 3);
        assert_eq!(img.dimensions(), (30, 20));
    }

    #[test]
    fn paliers_du_cache() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("photo.jpg");
        std::fs::write(&src, encode_jpeg(&gradient(3000, 2000)).unwrap()).unwrap();
        let cache = dir.path().join("cache");
        let out = generate(&src, MediaFormat::Image, 1, &cache, 42).unwrap();
        let sizes: Vec<_> = out
            .iter()
            .map(|g| (g.size, g.width.max(g.height)))
            .collect();
        assert_eq!(sizes, [(256, 256), (1024, 1024), (2048, 2048)]);
        for g in &out {
            assert!(cache_path(&cache, 42, g.size).exists());
        }
    }

    #[test]
    fn apercu_plus_petit_que_le_palier() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("petite.jpg");
        std::fs::write(&src, encode_jpeg(&gradient(1600, 1000)).unwrap()).unwrap();
        let out = generate(&src, MediaFormat::Image, 1, dir.path(), 1).unwrap();
        assert_eq!(out[2].width, 1600);
    }
}
