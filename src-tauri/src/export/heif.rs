//! HEIF par l'encodeur du système (décision 0007) : WIC sous Windows, qui
//! s'appuie sur les extensions HEIF et HEVC de Microsoft, ImageIO sous macOS.
//! Aucun encodeur HEVC n'est livré avec Afterdusk : sans celui du système, le
//! format est indisponible.
//!
//! Le système encode l'image et son profil ICC ; l'EXIF et le XMP sont
//! ajoutés ensuite par `heif_meta`, l'encodeur de Windows ignorant les
//! métadonnées qu'on lui donne.

use super::ExportError;
use super::encode::Embedded;
use super::heif_meta;
use super::render::Rendered;

/// Vrai si le système sait écrire du HEIF.
pub fn available() -> bool {
    imp::available()
}

/// Encode `img` en HEIF, avec son profil ICC, l'EXIF et le XMP.
pub fn encode(
    img: &Rendered,
    quality: u8,
    icc: &[u8],
    meta: &Embedded,
) -> Result<Vec<u8>, ExportError> {
    let rgb = img.to_u8();
    let file = imp::encode(img.width, img.height, &rgb, quality.clamp(1, 100), icc)?;
    heif_meta::embed(&file, meta.exif.as_deref(), meta.xmp.as_deref())
}

#[cfg(windows)]
mod imp {
    use std::mem::ManuallyDrop;

    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_ContainerFormatHeif, GUID_WICPixelFormat24bppRGB,
        IWICBitmapEncoder, IWICImagingFactory, WICBitmapEncoderNoCache, WICConvertBitmapSource,
    };
    use windows::Win32::System::Com::StructuredStorage::{
        CreateStreamOnHGlobal, IPropertyBag2, PROPBAG2,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, IStream,
        STREAM_SEEK_SET,
    };
    use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_R4};
    use windows::core::{GUID, PWSTR, w};

    use crate::export::ExportError;

    fn err(e: windows::core::Error) -> ExportError {
        ExportError::Encode(format!("HEIF : {e}"))
    }

    fn factory() -> windows::core::Result<IWICImagingFactory> {
        // Déjà initialisé dans ce fil : le résultat est sans importance.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
    }

    fn heif_encoder(f: &IWICImagingFactory) -> windows::core::Result<IWICBitmapEncoder> {
        unsafe { f.CreateEncoder(&GUID_ContainerFormatHeif, std::ptr::null()) }
    }

    pub fn available() -> bool {
        factory().and_then(|f| heif_encoder(&f)).is_ok()
    }

    fn read_all(stream: &IStream) -> windows::core::Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut buf = vec![0u8; 1 << 16];
        unsafe { stream.Seek(0, STREAM_SEEK_SET, None)? };
        loop {
            let mut read = 0;
            unsafe {
                stream
                    .Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut read))
                    .ok()?;
            }
            if read == 0 {
                return Ok(out);
            }
            out.extend_from_slice(&buf[..read as usize]);
        }
    }

    fn set_quality(bag: &IPropertyBag2, quality: u8) -> windows::core::Result<()> {
        let option = PROPBAG2 {
            pstrName: PWSTR(w!("ImageQuality").as_ptr().cast_mut()),
            ..Default::default()
        };
        let value = VARIANT {
            Anonymous: VARIANT_0 {
                Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                    vt: VT_R4,
                    Anonymous: VARIANT_0_0_0 {
                        fltVal: f32::from(quality) / 100.0,
                    },
                    ..Default::default()
                }),
            },
        };
        unsafe { bag.Write(1, &option, &value) }
    }

    pub fn encode(
        width: u32,
        height: u32,
        rgb: &[u8],
        quality: u8,
        icc: &[u8],
    ) -> Result<Vec<u8>, ExportError> {
        let f = factory().map_err(err)?;
        let encoder = heif_encoder(&f).map_err(|_| ExportError::HeifUnavailable)?;
        let stream = unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true) }.map_err(err)?;
        unsafe {
            encoder
                .Initialize(&stream, WICBitmapEncoderNoCache)
                .map_err(err)?;
            let mut frame = None;
            let mut bag = None;
            encoder.CreateNewFrame(&mut frame, &mut bag).map_err(err)?;
            let frame = frame.ok_or_else(|| ExportError::Encode("HEIF : cadre absent".into()))?;
            if let Some(bag) = &bag {
                set_quality(bag, quality).map_err(err)?;
            }
            frame.Initialize(bag.as_ref()).map_err(err)?;
            frame.SetSize(width, height).map_err(err)?;
            // Le codec choisit son format de pixels ; WIC convertit la source.
            let mut format: GUID = GUID_WICPixelFormat24bppRGB;
            frame.SetPixelFormat(&mut format).map_err(err)?;
            let ctx = f.CreateColorContext().map_err(err)?;
            ctx.InitializeFromMemory(icc).map_err(err)?;
            frame.SetColorContexts(&[Some(ctx)]).map_err(err)?;
            let bitmap = f
                .CreateBitmapFromMemory(width, height, &GUID_WICPixelFormat24bppRGB, width * 3, rgb)
                .map_err(err)?;
            let source = WICConvertBitmapSource(&format, &bitmap).map_err(err)?;
            frame.WriteSource(&source, std::ptr::null()).map_err(err)?;
            frame.Commit().map_err(err)?;
            encoder.Commit().map_err(err)?;
        }
        read_all(&stream).map_err(err)
    }

    /// Taille, pixel central et outil créateur lu dans le XMP.
    #[cfg(test)]
    type Reread = ((u32, u32), [u8; 3], Option<String>);

    /// Relit un HEIF avec le décodeur de Windows.
    #[cfg(test)]
    pub fn reread(bytes: &[u8]) -> windows::core::Result<Reread> {
        use windows::Win32::Graphics::Imaging::WICDecodeMetadataCacheOnDemand;
        use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
        use windows::Win32::System::Variant::VT_LPWSTR;

        let f = factory()?;
        unsafe {
            let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
            stream
                .Write(bytes.as_ptr().cast(), bytes.len() as u32, None)
                .ok()?;
            stream.Seek(0, STREAM_SEEK_SET, None)?;
            let decoder = f.CreateDecoderFromStream(
                &stream,
                std::ptr::null(),
                WICDecodeMetadataCacheOnDemand,
            )?;
            let frame = decoder.GetFrame(0)?;
            let (mut w, mut h) = (0, 0);
            frame.GetSize(&mut w, &mut h)?;
            let rgb = WICConvertBitmapSource(&GUID_WICPixelFormat24bppRGB, &frame)?;
            let mut row = vec![0u8; w as usize * 3];
            let rect = windows::Win32::Graphics::Imaging::WICRect {
                X: 0,
                Y: (h / 2) as i32,
                Width: w as i32,
                Height: 1,
            };
            rgb.CopyPixels(&rect, w * 3, &mut row)?;
            let mid = (w as usize / 2) * 3;
            let pixel = [row[mid], row[mid + 1], row[mid + 2]];
            let mut value = PROPVARIANT::default();
            let tool = frame
                .GetMetadataQueryReader()
                .and_then(|q| q.GetMetadataByName(w!("/xmp/xmp:CreatorTool"), &mut value))
                .ok()
                .and_then(|()| {
                    let v = &value.Anonymous.Anonymous;
                    (v.vt == VT_LPWSTR).then(|| v.Anonymous.pwszVal.to_string().ok())?
                });
            Ok(((w, h), pixel, tool))
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_core_foundation::{CFData, CFDictionary, CFMutableData, CFNumber, CFString, CFType};
    use objc2_core_graphics::{
        CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage,
        CGImageAlphaInfo,
    };
    use objc2_image_io::{CGImageDestination, kCGImageDestinationLossyCompressionQuality};

    use crate::export::ExportError;

    const HEIC: &str = "public.heic";

    fn fail(what: &str) -> ExportError {
        ExportError::Encode(format!("HEIF : {what}"))
    }

    pub fn available() -> bool {
        let Some(out) = CFMutableData::new(None, 0) else {
            return false;
        };
        let kind = CFString::from_static_str(HEIC);
        unsafe { CGImageDestination::with_data(&out, &kind, 1, None) }.is_some()
    }

    pub fn encode(
        width: u32,
        height: u32,
        rgb: &[u8],
        quality: u8,
        icc: &[u8],
    ) -> Result<Vec<u8>, ExportError> {
        let (w, h) = (width as usize, height as usize);
        let profile = CFData::from_bytes(icc);
        let profile: &CFType = &profile;
        let space =
            unsafe { CGColorSpace::with_icc_data(Some(profile)) }.ok_or_else(|| fail("profil"))?;
        let pixels = CFData::from_bytes(rgb);
        let provider = CGDataProvider::with_cf_data(Some(&pixels)).ok_or_else(|| fail("pixels"))?;
        let image = unsafe {
            CGImage::new(
                w,
                h,
                8,
                24,
                w * 3,
                Some(&space),
                CGBitmapInfo(CGImageAlphaInfo::None.0),
                Some(&provider),
                std::ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
        }
        .ok_or_else(|| fail("image"))?;
        let out = CFMutableData::new(None, 0).ok_or_else(|| fail("tampon"))?;
        let kind = CFString::from_static_str(HEIC);
        let dest = unsafe { CGImageDestination::with_data(&out, &kind, 1, None) }
            .ok_or(ExportError::HeifUnavailable)?;
        let key: &CFString = unsafe { kCGImageDestinationLossyCompressionQuality };
        let value = CFNumber::new_f64(f64::from(quality) / 100.0);
        let options = CFDictionary::<CFString, CFNumber>::from_slices(&[key], &[&*value]);
        unsafe { dest.add_image(&image, Some(options.as_opaque())) };
        if !unsafe { dest.finalize() } {
            return Err(fail("écriture"));
        }
        Ok(out.to_vec())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use crate::export::ExportError;

    pub fn available() -> bool {
        false
    }

    pub fn encode(
        _width: u32,
        _height: u32,
        _rgb: &[u8],
        _quality: u8,
        _icc: &[u8],
    ) -> Result<Vec<u8>, ExportError> {
        Err(ExportError::HeifUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::color::ColorSpace;
    use crate::export::heif_meta::find;

    #[test]
    fn heif_du_systeme_avec_profil_et_metadonnees() {
        if !available() {
            eprintln!("pas d'encodeur HEIF sur ce système : test sauté");
            return;
        }
        let (w, h) = (96u32, 64u32);
        let img = Rendered {
            width: w,
            height: h,
            rgb: (0..w * h)
                .flat_map(|i| [(i % w) as f32 / w as f32, 0.5, 0.2])
                .collect(),
        };
        let icc = ColorSpace::DisplayP3.icc().unwrap();
        let meta = Embedded {
            exif: Some(b"II*\0\x08\0\0\0\0\0\0\0\0\0".to_vec()),
            xmp: Some(
                "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF \
                 xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
                 <rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" \
                 xmp:CreatorTool=\"Afterdusk\"/></rdf:RDF></x:xmpmeta>"
                    .into(),
            ),
        };
        let bytes = encode(&img, 80, &icc, &meta).unwrap();
        assert_eq!(&bytes[4..8], b"ftyp");
        // Dimensions de l'image principale (boîte ispe).
        let ispe = find(&bytes, b"ispe").expect("boîte ispe");
        let dim = |o: usize| u32::from_be_bytes(bytes[o..o + 4].try_into().unwrap());
        assert_eq!((dim(ispe + 8), dim(ispe + 12)), (w, h));
        assert!(
            find(&bytes, b"colr").is_some(),
            "espace colorimétrique absent"
        );
        assert!(find(&bytes, b"Afterdusk").is_some(), "XMP absent");
        assert!(find(&bytes, b"II*\0").is_some(), "EXIF absent");
        // Le décodeur du système relit l'image et le XMP ajouté.
        #[cfg(windows)]
        {
            let (size, pixel, tool) = imp::reread(&bytes).unwrap();
            assert_eq!(size, (w, h));
            // Pixel central : rouge à mi-course, vert 0,5, bleu 0,2 (en Display P3).
            assert!(pixel[0] > 90 && pixel[0] < 170, "{pixel:?}");
            assert!(pixel[2] < 110, "{pixel:?}");
            assert_eq!(tool.as_deref(), Some("Afterdusk"));
        }
    }
}
