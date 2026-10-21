//! Écriture des fichiers exportés : JPEG, PNG et TIFF (8 ou 16 bits), AVIF.
//! Profil ICC incorporé ; écriture dans un fichier temporaire renommé à la
//! fin, pour ne jamais laisser un fichier tronqué sous son nom final.

use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ExportError;
use super::render::Rendered;
use crate::import::copy::part_path;

/// Format de fichier rendu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Format {
    Jpeg {
        quality: u8,
    },
    Png {
        sixteen: bool,
    },
    Tiff {
        sixteen: bool,
    },
    /// AVIF en sRGB.
    Avif {
        quality: u8,
    },
    /// HEIF par l'encodeur du système (voir `heif`).
    Heif {
        quality: u8,
    },
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg { .. } => "jpg",
            Self::Png { .. } => "png",
            Self::Tiff { .. } => "tif",
            Self::Avif { .. } => "avif",
            Self::Heif { .. } => "heic",
        }
    }
}

/// Métadonnées à incorporer (préparées par `metadata`).
#[derive(Debug, Clone, Default)]
pub struct Embedded {
    /// Bloc EXIF (TIFF commençant par « II* » ou « MM* »).
    pub exif: Option<Vec<u8>>,
    /// Paquet XMP.
    pub xmp: Option<String>,
}

fn jpeg(img: &Rendered, quality: u8, icc: &[u8], meta: &Embedded) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, quality.clamp(1, 100));
    let enc = |e: jpeg_encoder::EncodingError| ExportError::Encode(e.to_string());
    if let Some(exif) = &meta.exif {
        let mut data = b"Exif\0\0".to_vec();
        data.extend_from_slice(exif);
        encoder.add_app_segment(1, data).map_err(enc)?;
    }
    if let Some(xmp) = &meta.xmp {
        let mut data = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        data.extend_from_slice(xmp.as_bytes());
        encoder.add_app_segment(1, data).map_err(enc)?;
    }
    encoder.add_icc_profile(icc).map_err(enc)?;
    let (w, h) = (
        u16::try_from(img.width).map_err(|_| ExportError::TooLarge)?,
        u16::try_from(img.height).map_err(|_| ExportError::TooLarge)?,
    );
    encoder
        .encode(&img.to_u8(), w, h, jpeg_encoder::ColorType::Rgb)
        .map_err(enc)?;
    Ok(out)
}

fn png(img: &Rendered, sixteen: bool, icc: &[u8], meta: &Embedded) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::new();
    {
        let enc = |e: png::EncodingError| ExportError::Encode(e.to_string());
        let mut info = png::Info::with_size(img.width, img.height);
        info.color_type = png::ColorType::Rgb;
        info.bit_depth = if sixteen {
            png::BitDepth::Sixteen
        } else {
            png::BitDepth::Eight
        };
        info.icc_profile = Some(Cow::Borrowed(icc));
        let mut encoder = png::Encoder::with_info(&mut out, info).map_err(enc)?;
        encoder.set_compression(png::Compression::Balanced);
        let mut writer = encoder.write_header().map_err(enc)?;
        if let Some(xmp) = &meta.xmp {
            writer
                .write_text_chunk(&png::text_metadata::ITXtChunk::new(
                    "XML:com.adobe.xmp",
                    xmp.clone(),
                ))
                .map_err(enc)?;
        }
        if let Some(exif) = &meta.exif {
            writer.write_chunk(png::chunk::eXIf, exif).map_err(enc)?;
        }
        if sixteen {
            let data: Vec<u8> = img.to_u16().iter().flat_map(|v| v.to_be_bytes()).collect();
            writer.write_image_data(&data).map_err(enc)?;
        } else {
            writer.write_image_data(&img.to_u8()).map_err(enc)?;
        }
        writer.finish().map_err(enc)?;
    }
    Ok(out)
}

fn tiff(
    img: &Rendered,
    sixteen: bool,
    icc: &[u8],
    meta: &Embedded,
) -> Result<Vec<u8>, ExportError> {
    use tiff::encoder::{TiffEncoder, colortype};
    use tiff::tags::Tag;
    let enc = |e: tiff::TiffError| ExportError::Encode(e.to_string());
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut out).map_err(enc)?;
        macro_rules! write {
            ($ty:ty, $data:expr) => {{
                let mut image = encoder
                    .new_image::<$ty>(img.width, img.height)
                    .map_err(enc)?;
                image
                    .encoder()
                    .write_tag(Tag::IccProfile, icc)
                    .map_err(enc)?;
                if let Some(xmp) = &meta.xmp {
                    image
                        .encoder()
                        .write_tag(Tag::Unknown(700), xmp.as_bytes())
                        .map_err(enc)?;
                }
                image.write_data(&$data).map_err(enc)?;
            }};
        }
        if sixteen {
            write!(colortype::RGB16, img.to_u16());
        } else {
            write!(colortype::RGB8, img.to_u8());
        }
    }
    Ok(out.into_inner())
}

fn avif(img: &Rendered, quality: u8) -> Result<Vec<u8>, ExportError> {
    let pixels: Vec<ravif::RGB8> = img
        .to_u8()
        .chunks(3)
        .map(|p| ravif::RGB8::new(p[0], p[1], p[2]))
        .collect();
    let encoded = ravif::Encoder::new()
        .with_quality(f32::from(quality.clamp(1, 100)))
        .with_speed(6)
        .encode_rgb(ravif::Img::new(
            &pixels[..],
            img.width as usize,
            img.height as usize,
        ))
        .map_err(|e| ExportError::Encode(e.to_string()))?;
    Ok(encoded.avif_file)
}

/// Encode `img` et l'écrit à `path` (qui ne doit pas exister).
pub fn write(
    img: &Rendered,
    format: Format,
    icc: &[u8],
    meta: &Embedded,
    path: &Path,
) -> Result<(), ExportError> {
    if path.exists() {
        return Err(ExportError::TargetExists(path.to_path_buf()));
    }
    let bytes = match format {
        Format::Jpeg { quality } => jpeg(img, quality, icc, meta)?,
        Format::Png { sixteen } => png(img, sixteen, icc, meta)?,
        Format::Tiff { sixteen } => tiff(img, sixteen, icc, meta)?,
        Format::Avif { quality } => avif(img, quality)?,
        Format::Heif { quality } => super::heif::encode(img, quality, icc, meta)?,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = part_path(path);
    {
        let mut file = BufWriter::new(File::create(&part)?);
        file.write_all(&bytes)?;
        file.into_inner()
            .map_err(|e| ExportError::Io(e.into_error()))?
            .sync_all()?;
    }
    fs::rename(&part, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::color::ColorSpace;

    fn gradient(w: u32, h: u32) -> Rendered {
        Rendered {
            width: w,
            height: h,
            rgb: (0..w * h)
                .flat_map(|i| {
                    let x = (i % w) as f32 / w as f32;
                    [x, 0.5, 1.0 - x]
                })
                .collect(),
        }
    }

    #[test]
    fn chaque_format_se_relit() {
        let dir = tempfile::tempdir().unwrap();
        let img = gradient(64, 48);
        let icc = ColorSpace::AdobeRgb.icc().unwrap();
        let meta = Embedded::default();
        for (format, name) in [
            (Format::Jpeg { quality: 90 }, "a.jpg"),
            (Format::Png { sixteen: false }, "b.png"),
            (Format::Png { sixteen: true }, "c.png"),
            (Format::Tiff { sixteen: false }, "d.tif"),
            (Format::Tiff { sixteen: true }, "e.tif"),
        ] {
            let path = dir.path().join(name);
            write(&img, format, &icc, &meta, &path).unwrap();
            let decoded = image::open(&path).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (64, 48), "{name}");
            let px = decoded.to_rgb8();
            let p = px.get_pixel(60, 10);
            assert!(p[0] > 200 && p[2] < 50, "{name} : {p:?}");
            // Profil incorporé.
            let embedded = if name.ends_with(".tif") {
                let mut decoder = tiff::decoder::Decoder::new(File::open(&path).unwrap()).unwrap();
                decoder.get_tag_u8_vec(tiff::tags::Tag::IccProfile).ok()
            } else {
                use image::ImageDecoder;
                let mut decoder = image::ImageReader::open(&path)
                    .unwrap()
                    .with_guessed_format()
                    .unwrap()
                    .into_decoder()
                    .unwrap();
                decoder.icc_profile().unwrap()
            };
            assert_eq!(embedded.as_deref(), Some(&icc[..]), "{name}");
            // Jamais par-dessus un fichier existant.
            assert!(matches!(
                write(&img, format, &icc, &meta, &path),
                Err(ExportError::TargetExists(_))
            ));
        }
        let avif_path = dir.path().join("f.avif");
        write(&img, Format::Avif { quality: 70 }, &icc, &meta, &avif_path).unwrap();
        let bytes = fs::read(&avif_path).unwrap();
        assert_eq!(&bytes[4..12], b"ftypavif");
    }
}
