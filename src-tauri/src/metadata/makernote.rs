//! Compteur de déclenchements, lu dans les notes du fabricant quand la marque
//! l'y écrit en clair : Nikon (ShutterCount, 0x00a7) et Fujifilm (ImageCount,
//! 0x1438). Canon et Sony le rangent dans des blocs propriétaires ou chiffrés ;
//! pour eux, l'estimation d'après les photos importées suffit.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Notes du fabricant plus grandes : fichier abîmé.
const MAX_MAKERNOTE: u32 = 4 << 20;

#[derive(Debug, Clone, Copy)]
enum Order {
    Little,
    Big,
}

impl Order {
    fn from_header(b: &[u8]) -> Option<Self> {
        match b.get(0..2)? {
            b"II" => Some(Self::Little),
            b"MM" => Some(Self::Big),
            _ => None,
        }
    }

    fn u16(self, b: &[u8], at: usize) -> Option<u16> {
        let v: [u8; 2] = b.get(at..at + 2)?.try_into().ok()?;
        Some(match self {
            Self::Little => u16::from_le_bytes(v),
            Self::Big => u16::from_be_bytes(v),
        })
    }

    fn u32(self, b: &[u8], at: usize) -> Option<u32> {
        let v: [u8; 4] = b.get(at..at + 4)?.try_into().ok()?;
        Some(match self {
            Self::Little => u32::from_le_bytes(v),
            Self::Big => u32::from_be_bytes(v),
        })
    }
}

/// Entrée d'un IFD : type, nombre de valeurs et les quatre octets de valeur
/// (la valeur elle-même si elle tient, sinon son décalage).
struct Entry {
    kind: u16,
    count: u32,
    value: usize,
}

/// Cherche `tag` dans l'IFD qui commence à `ifd` dans `b`.
fn find(b: &[u8], order: Order, ifd: usize, tag: u16) -> Option<Entry> {
    let n = usize::from(order.u16(b, ifd)?);
    (0..n).find_map(|i| {
        let at = ifd + 2 + 12 * i;
        (order.u16(b, at)? == tag).then_some(Entry {
            kind: order.u16(b, at + 2)?,
            count: order.u32(b, at + 4)?,
            value: at + 8,
        })
    })
}

/// Première valeur entière (SHORT ou LONG) d'une entrée.
fn uint(b: &[u8], order: Order, e: &Entry) -> Option<u32> {
    match e.kind {
        3 => order.u16(b, e.value).map(u32::from),
        4 => order.u32(b, e.value),
        _ => None,
    }
}

/// Compteur de déclenchements écrit dans les notes du fabricant, s'il y est.
pub fn shutter_count(makernote: &[u8]) -> Option<u32> {
    if makernote.starts_with(b"Nikon\0") {
        // En-tête, version, puis un TIFF complet dont les décalages partent
        // de son propre début.
        let tiff = makernote.get(10..)?;
        let order = Order::from_header(tiff)?;
        let ifd = order.u32(tiff, 4)? as usize;
        let e = find(tiff, order, ifd, 0x00a7)?;
        uint(tiff, order, &e).filter(|&n| n > 0)
    } else if makernote.starts_with(b"FUJIFILM") {
        // Toujours en petit-boutiste, décalages depuis le début des notes.
        let order = Order::Little;
        let ifd = order.u32(makernote, 8)? as usize;
        let e = find(makernote, order, ifd, 0x1438)?;
        // Le bit de poids fort ne fait pas partie du compte.
        uint(makernote, order, &e)
            .map(|n| n & 0x7fff)
            .filter(|&n| n > 0)
    } else {
        None
    }
}

/// TIFF dans un fichier, lu par petits morceaux : un RAW pèse des dizaines de
/// mégaoctets et les notes du fabricant sont au début.
struct FileTiff {
    file: File,
    base: u64,
    order: Order,
}

impl FileTiff {
    fn open(mut file: File, base: u64) -> Option<Self> {
        let header = read_at(&mut file, base, 8)?;
        let order = Order::from_header(&header)?;
        (order.u16(&header, 2)? == 42).then_some(Self { file, base, order })
    }

    fn read(&mut self, offset: u32, len: usize) -> Option<Vec<u8>> {
        read_at(&mut self.file, self.base + u64::from(offset), len)
    }

    fn u32(&mut self, offset: u32) -> Option<u32> {
        let b = self.read(offset, 4)?;
        self.order.u32(&b, 0)
    }

    /// Valeur (ou décalage) de `tag` dans l'IFD à `ifd`, avec son nombre.
    fn entry(&mut self, ifd: u32, tag: u16) -> Option<(u32, u32)> {
        let n = self.read(ifd, 2)?;
        let n = usize::from(self.order.u16(&n, 0)?);
        let b = self.read(ifd, 2 + 12 * n)?;
        let e = find(&b, self.order, 0, tag)?;
        Some((e.count, self.order.u32(&b, e.value)?))
    }

    fn makernote(&mut self) -> Option<Vec<u8>> {
        let ifd0 = self.u32(4)?;
        let (_, exif) = self.entry(ifd0, 0x8769)?;
        let (count, offset) = self.entry(exif, 0x927c)?;
        if count > MAX_MAKERNOTE {
            return None;
        }
        self.read(offset, count as usize)
    }
}

fn read_at(file: &mut File, pos: u64, len: usize) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(pos)).ok()?;
    let mut buf = vec![0; len];
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// Début du TIFF de l'EXIF dans le JPEG intégré à un RAF.
fn raf_exif_base(file: &mut File) -> Option<u64> {
    let header = read_at(file, 0, 92)?;
    if !header.starts_with(b"FUJIFILMCCD-RAW") {
        return None;
    }
    let jpeg = u64::from(Order::Big.u32(&header, 84)?);
    // Segments du JPEG jusqu'à APP1 « Exif ».
    let mut at = jpeg + 2;
    for _ in 0..16 {
        let seg = read_at(file, at, 10)?;
        let (marker, len) = (Order::Big.u16(&seg, 0)?, Order::Big.u16(&seg, 2)?);
        if marker == 0xffe1 && &seg[4..10] == b"Exif\0\0" {
            return Some(at + 10);
        }
        if marker & 0xff00 != 0xff00 || marker == 0xffda {
            return None;
        }
        at += 2 + u64::from(len);
    }
    None
}

/// Notes du fabricant d'un RAW en TIFF (NEF…) ou d'un RAF.
pub fn read_raw(path: &Path) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let base = raf_exif_base(&mut file).unwrap_or(0);
    FileTiff::open(file, base)?.makernote()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Notes Nikon minimales : en-tête, TIFF big-endian, un IFD d'une entrée.
    fn nikon(count: u32) -> Vec<u8> {
        let mut b = b"Nikon\0\x02\x10\0\0MM\0\x2a\0\0\0\x08".to_vec();
        b.extend_from_slice(&1u16.to_be_bytes());
        b.extend_from_slice(&0x00a7u16.to_be_bytes());
        b.extend_from_slice(&4u16.to_be_bytes());
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(&count.to_be_bytes());
        b
    }

    fn fuji(count: u16) -> Vec<u8> {
        let mut b = b"FUJIFILM".to_vec();
        b.extend_from_slice(&12u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&0x1438u16.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&count.to_le_bytes());
        b.extend_from_slice(&[0, 0]);
        b
    }

    #[test]
    fn compteurs_nikon_et_fujifilm() {
        assert_eq!(shutter_count(&nikon(48_213)), Some(48_213));
        assert_eq!(shutter_count(&fuji(0x8000 | 1234)), Some(1234));
        assert_eq!(shutter_count(&nikon(0)), None);
        assert_eq!(shutter_count(b"Canon"), None);
        // Tronqué : pas de panique, rien de lu.
        assert_eq!(shutter_count(&nikon(5)[..20]), None);
    }

    #[test]
    fn compteurs_des_echantillons() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-assets");
        let count = |name: &str| {
            read_raw(&assets.join(name))
                .as_deref()
                .and_then(shutter_count)
        };
        assert_eq!(count("nikon_z6.NEF"), Some(739));
        assert_eq!(count("fujifilm_x-t3.RAF"), Some(3850));
        // Sony chiffre le sien, Canon le range ailleurs.
        assert_eq!(count("sony_ilce-7m3.ARW"), None);
        assert_eq!(count("canon_eos_r6.CR3"), None);
    }
}
