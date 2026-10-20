//! Métadonnées des fichiers exportés : EXIF (prise de vue, auteur, GPS) et
//! XMP (note, mots-clés, auteur, droits), filtrés selon le niveau de
//! nettoyage choisi. Les pixels exportés sont déjà orientés : l'orientation
//! EXIF vaut 1. Les réglages de retouche d'Afterdusk ne sont jamais exportés.

use exif::experimental::Writer;
use exif::{Field, In, Rational, SRational, Tag, Value};
use serde::{Deserialize, Serialize};
use xmp_toolkit::{ToStringOptions, XmpMeta, XmpValue, xmp_ns};

use super::ExportError;
use super::encode::Embedded;
use crate::metadata::media::MediaMetadata;
use crate::metadata::xmp::SidecarData;

/// Nettoyage des métadonnées (cahier des charges, 4.6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataPolicy {
    /// Tout garder.
    #[default]
    All,
    /// Retirer la position GPS.
    NoGps,
    /// Retirer le GPS et le numéro de série du boîtier.
    NoGpsSerial,
    /// Ne garder que l'auteur et le copyright.
    CopyrightOnly,
    /// Tout retirer.
    None,
}

/// Ce que l'on sait d'une photo : EXIF de l'original et XMP annexe.
#[derive(Debug, Clone, Default)]
pub struct PhotoMeta {
    pub camera: MediaMetadata,
    pub sidecar: Option<SidecarData>,
}

fn ascii(tag: Tag, text: &str) -> Field {
    Field {
        tag,
        ifd_num: In::PRIMARY,
        value: Value::Ascii(vec![text.trim().as_bytes().to_vec()]),
    }
}

fn rational(v: f64) -> Rational {
    // Précision au millième, sans dépasser u32.
    let denom = 1000u32;
    Rational {
        num: (v.max(0.0) * f64::from(denom))
            .round()
            .min(f64::from(u32::MAX)) as u32,
        denom,
    }
}

/// Durée d'exposition en fraction lisible (1/250, 2/1).
fn exposure_time(seconds: f64) -> Rational {
    if seconds > 0.0 && seconds < 1.0 {
        Rational {
            num: 1,
            denom: (1.0 / seconds).round().max(1.0) as u32,
        }
    } else {
        rational(seconds)
    }
}

/// Degrés décimaux en degrés, minutes, secondes.
fn dms(value: f64) -> Vec<Rational> {
    let v = value.abs();
    let d = v.trunc();
    let m = ((v - d) * 60.0).trunc();
    let s = ((v - d) * 60.0 - m) * 60.0;
    vec![
        Rational {
            num: d as u32,
            denom: 1,
        },
        Rational {
            num: m as u32,
            denom: 1,
        },
        rational(s),
    ]
}

/// Champs EXIF selon le niveau de nettoyage.
fn exif_fields(meta: &PhotoMeta, policy: MetadataPolicy) -> Vec<Field> {
    let mut fields = Vec::new();
    if policy == MetadataPolicy::None {
        return fields;
    }
    let side = meta.sidecar.as_ref();
    if let Some(creator) = side.and_then(|s| s.creator.as_deref()) {
        fields.push(ascii(Tag::Artist, creator));
    }
    if let Some(rights) = side.and_then(|s| s.rights.as_deref()) {
        fields.push(ascii(Tag::Copyright, rights));
    }
    if policy == MetadataPolicy::CopyrightOnly {
        return fields;
    }
    let c = &meta.camera;
    fields.push(ascii(Tag::Software, "Afterdusk"));
    fields.push(Field {
        tag: Tag::Orientation,
        ifd_num: In::PRIMARY,
        value: Value::Short(vec![1]),
    });
    if let Some(v) = &c.make {
        fields.push(ascii(Tag::Make, v));
    }
    if let Some(v) = &c.model {
        fields.push(ascii(Tag::Model, v));
    }
    if let Some(v) = &c.lens {
        fields.push(ascii(Tag::LensModel, v));
    }
    if let Some(at) = &c.captured_at {
        // « AAAA-MM-JJTHH:MM:SS » vers « AAAA:MM:JJ HH:MM:SS ».
        let exif_date = at.replacen('-', ":", 2).replacen('T', " ", 1);
        fields.push(ascii(Tag::DateTimeOriginal, &exif_date));
        fields.push(ascii(Tag::DateTime, &exif_date));
    }
    if let Some(v) = &c.utc_offset {
        fields.push(ascii(Tag::OffsetTimeOriginal, v));
    }
    if let Some(v) = c.shutter {
        fields.push(Field {
            tag: Tag::ExposureTime,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![exposure_time(f64::from(v))]),
        });
    }
    if let Some(v) = c.aperture {
        fields.push(Field {
            tag: Tag::FNumber,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![rational(f64::from(v))]),
        });
    }
    if let Some(v) = c.iso {
        fields.push(Field {
            tag: Tag::PhotographicSensitivity,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![v.round().clamp(0.0, 65535.0) as u16]),
        });
    }
    if let Some(v) = c.focal_length {
        fields.push(Field {
            tag: Tag::FocalLength,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![rational(f64::from(v))]),
        });
    }
    if let Some(v) = c.focal_length_35mm {
        fields.push(Field {
            tag: Tag::FocalLengthIn35mmFilm,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![v]),
        });
    }
    if let Some(v) = c.exposure_compensation {
        fields.push(Field {
            tag: Tag::ExposureBiasValue,
            ifd_num: In::PRIMARY,
            value: Value::SRational(vec![SRational {
                num: (v * 100.0).round() as i32,
                denom: 100,
            }]),
        });
    }
    if policy == MetadataPolicy::All || policy == MetadataPolicy::NoGps {
        if let Some(v) = &c.body_serial {
            fields.push(ascii(Tag::BodySerialNumber, v));
        }
    }
    if policy == MetadataPolicy::All {
        if let Some(g) = &c.gps {
            let gps = |tag, value| Field {
                tag,
                ifd_num: In::PRIMARY,
                value,
            };
            fields.push(gps(Tag::GPSVersionID, Value::Byte(vec![2, 3, 0, 0])));
            let lat_ref = if g.latitude < 0.0 { "S" } else { "N" };
            let lon_ref = if g.longitude < 0.0 { "W" } else { "E" };
            fields.push(ascii(Tag::GPSLatitudeRef, lat_ref));
            fields.push(gps(Tag::GPSLatitude, Value::Rational(dms(g.latitude))));
            fields.push(ascii(Tag::GPSLongitudeRef, lon_ref));
            fields.push(gps(Tag::GPSLongitude, Value::Rational(dms(g.longitude))));
            if let Some(alt) = g.altitude {
                fields.push(gps(
                    Tag::GPSAltitudeRef,
                    Value::Byte(vec![u8::from(alt < 0.0)]),
                ));
                fields.push(gps(
                    Tag::GPSAltitude,
                    Value::Rational(vec![rational(alt.abs())]),
                ));
            }
        }
    }
    fields
}

fn exif_block(fields: &[Field]) -> Result<Option<Vec<u8>>, ExportError> {
    if fields.is_empty() {
        return Ok(None);
    }
    let mut writer = Writer::new();
    for f in fields {
        writer.push_field(f);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    writer
        .write(&mut out, false)
        .map_err(|e| ExportError::Metadata(e.to_string()))?;
    Ok(Some(out.into_inner()))
}

fn xmp_packet(meta: &PhotoMeta, policy: MetadataPolicy) -> Result<Option<String>, ExportError> {
    if policy == MetadataPolicy::None {
        return Ok(None);
    }
    let err = |e: xmp_toolkit::XmpError| ExportError::Metadata(e.to_string());
    let mut xmp = XmpMeta::new().map_err(err)?;
    let side = meta.sidecar.clone().unwrap_or_default();
    if let Some(creator) = &side.creator {
        let seq = XmpValue::from("creator")
            .set_is_array(true)
            .set_is_ordered(true);
        xmp.append_array_item(xmp_ns::DC, &seq, &creator.as_str().into())
            .map_err(err)?;
    }
    if let Some(rights) = &side.rights {
        xmp.set_localized_text(xmp_ns::DC, "rights", None, "x-default", rights)
            .map_err(err)?;
    }
    if policy != MetadataPolicy::CopyrightOnly {
        xmp.set_property(xmp_ns::XMP, "CreatorTool", &"Afterdusk".into())
            .map_err(err)?;
        if side.rating > 0 {
            xmp.set_property_i32(xmp_ns::XMP, "Rating", &i32::from(side.rating).into())
                .map_err(err)?;
        }
        let bag = XmpValue::from("subject").set_is_array(true);
        for k in &side.keywords {
            xmp.append_array_item(xmp_ns::DC, &bag, &k.as_str().into())
                .map_err(err)?;
        }
    }
    let text = xmp
        .to_string_with_options(ToStringOptions::default())
        .map_err(err)?;
    Ok(Some(text))
}

/// EXIF et XMP à incorporer au fichier exporté.
pub fn embedded(meta: &PhotoMeta, policy: MetadataPolicy) -> Result<Embedded, ExportError> {
    Ok(Embedded {
        exif: exif_block(&exif_fields(meta, policy))?,
        xmp: xmp_packet(meta, policy)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::Gps;

    fn sample() -> PhotoMeta {
        PhotoMeta {
            camera: MediaMetadata {
                make: Some("Sony".into()),
                model: Some("ILCE-7M3".into()),
                lens: Some("FE 16-35mm F2.8 GM".into()),
                body_serial: Some("1234567".into()),
                iso: Some(400.0),
                shutter: Some(1.0 / 250.0),
                aperture: Some(4.0),
                focal_length: Some(32.0),
                captured_at: Some("2018-03-13T14:05:09".into()),
                gps: Some(Gps {
                    latitude: 48.8584,
                    longitude: -2.2945,
                    altitude: Some(35.0),
                }),
                ..MediaMetadata::default()
            },
            sidecar: Some(SidecarData {
                rating: 4,
                keywords: vec!["Bois".into(), "Hiver".into()],
                creator: Some("Lewis Bedar".into()),
                rights: Some("© 2026 Lewis Bedar".into()),
                ..SidecarData::default()
            }),
        }
    }

    fn read(e: &Embedded) -> exif::Exif {
        exif::Reader::new()
            .read_raw(e.exif.clone().unwrap())
            .unwrap()
    }

    #[test]
    fn tout_garder() {
        let e = embedded(&sample(), MetadataPolicy::All).unwrap();
        let x = read(&e);
        let s = |tag| {
            x.get_field(tag, In::PRIMARY)
                .map(|f| f.display_value().to_string())
        };
        assert_eq!(s(Tag::Model).as_deref(), Some("\"ILCE-7M3\""));
        assert_eq!(s(Tag::ExposureTime).as_deref(), Some("1/250"));
        assert_eq!(
            s(Tag::DateTimeOriginal).as_deref(),
            Some("2018-03-13 14:05:09")
        );
        assert!(s(Tag::BodySerialNumber).is_some());
        assert!(s(Tag::GPSLatitude).is_some());
        assert_eq!(s(Tag::GPSLongitudeRef).as_deref(), Some("W"));
        assert_eq!(
            s(Tag::Orientation).as_deref(),
            Some("row 0 at top and column 0 at left")
        );
        let xmp = e.xmp.unwrap();
        assert!(xmp.contains("Hiver") && xmp.contains("Lewis Bedar"));
        assert!(!xmp.contains("afterdusk:Develop"));
    }

    #[test]
    fn niveaux_de_nettoyage() {
        let meta = sample();
        let has = |policy, tag| {
            let e = embedded(&meta, policy).unwrap();
            e.exif
                .map(|b| exif::Reader::new().read_raw(b).unwrap())
                .is_some_and(|x| x.get_field(tag, In::PRIMARY).is_some())
        };
        assert!(!has(MetadataPolicy::NoGps, Tag::GPSLatitude));
        assert!(has(MetadataPolicy::NoGps, Tag::BodySerialNumber));
        assert!(!has(MetadataPolicy::NoGpsSerial, Tag::BodySerialNumber));
        assert!(has(MetadataPolicy::NoGpsSerial, Tag::Model));
        assert!(has(MetadataPolicy::CopyrightOnly, Tag::Copyright));
        assert!(!has(MetadataPolicy::CopyrightOnly, Tag::Model));
        let none = embedded(&meta, MetadataPolicy::None).unwrap();
        assert!(none.exif.is_none() && none.xmp.is_none());
        let only = embedded(&meta, MetadataPolicy::CopyrightOnly)
            .unwrap()
            .xmp
            .unwrap();
        assert!(only.contains("Lewis Bedar") && !only.contains("Hiver"));
    }
}
