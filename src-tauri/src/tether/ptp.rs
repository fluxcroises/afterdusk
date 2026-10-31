//! Protocole de l'appareil photo (PTP, ISO 15740), réduit aux lectures :
//! fiche d'identité, mémoires, liste, fiche et contenu des objets, valeur
//! d'une propriété. Le transport (Windows Portable Devices, ou le boîtier
//! factice des tests) ne reçoit que des `Operation`, qui ne se construisent
//! qu'à partir de `ALLOWED`.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::TetherError;
use crate::import::copy::part_path;

pub const GET_DEVICE_INFO: u16 = 0x1001;
pub const GET_STORAGE_IDS: u16 = 0x1004;
pub const GET_OBJECT_HANDLES: u16 = 0x1007;
pub const GET_OBJECT_INFO: u16 = 0x1008;
pub const GET_OBJECT: u16 = 0x1009;
pub const GET_DEVICE_PROP_VALUE: u16 = 0x1015;

/// Seules opérations envoyées au boîtier : toutes de lecture (décision 0016).
pub const ALLOWED: [u16; 6] = [
    GET_DEVICE_INFO,
    GET_STORAGE_IDS,
    GET_OBJECT_HANDLES,
    GET_OBJECT_INFO,
    GET_OBJECT,
    GET_DEVICE_PROP_VALUE,
];

pub const RESPONSE_OK: u16 = 0x2001;

/// Propriété Fujifilm des changements en prise de vue reliée : une liste de
/// paires (propriété, valeur) changées depuis la dernière lecture.
pub const FUJI_CHANGES: u32 = 0xD212;
/// Propriété Fujifilm : photos que la carte peut encore recevoir.
pub const FUJI_REMAINING_SHOTS: u32 = 0xD20E;

/// Taille d'objet inconnue (fichier de plus de 4 Go).
const SIZE_UNKNOWN: u32 = 0xFFFF_FFFF;

/// Opération PTP autorisée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation(u16);

impl Operation {
    pub fn new(code: u16) -> Result<Self, TetherError> {
        if ALLOWED.contains(&code) {
            Ok(Self(code))
        } else {
            Err(TetherError::Refused(code))
        }
    }

    pub fn code(self) -> u16 {
        self.0
    }
}

/// Chemin vers le boîtier.
pub trait Transport {
    /// Envoie une opération qui renvoie des données, écrites au fil de l'eau
    /// dans `sink` ; rend le code de réponse du boîtier.
    fn read(&self, op: Operation, params: &[u32], sink: &mut dyn Write)
    -> Result<u16, TetherError>;
}

/// Lecteur de données PTP (petit-boutiste).
pub struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], TetherError> {
        let end = self.at + N;
        let slice = self
            .data
            .get(self.at..end)
            .ok_or(TetherError::Malformed("données trop courtes"))?;
        self.at = end;
        let mut out = [0; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, TetherError> {
        Ok(self.bytes::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16, TetherError> {
        Ok(u16::from_le_bytes(self.bytes()?))
    }

    pub fn u32(&mut self) -> Result<u32, TetherError> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }

    pub fn skip(&mut self, n: usize) -> Result<(), TetherError> {
        if self.at + n > self.data.len() {
            return Err(TetherError::Malformed("données trop courtes"));
        }
        self.at += n;
        Ok(())
    }

    pub fn string(&mut self) -> Result<String, TetherError> {
        let n = self.u8()? as usize;
        let chars = (0..n).map(|_| self.u16()).collect::<Result<Vec<_>, _>>()?;
        Ok(String::from_utf16_lossy(&chars)
            .trim_end_matches('\0')
            .to_owned())
    }

    pub fn array16(&mut self) -> Result<Vec<u16>, TetherError> {
        let n = self.u32()? as usize;
        (0..n).map(|_| self.u16()).collect()
    }

    pub fn array32(&mut self) -> Result<Vec<u32>, TetherError> {
        let n = self.u32()? as usize;
        (0..n).map(|_| self.u32()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub firmware: String,
    pub serial: String,
    pub operations: Vec<u16>,
    pub properties: Vec<u16>,
}

impl DeviceInfo {
    pub fn parse(data: &[u8]) -> Result<Self, TetherError> {
        let mut r = Reader::new(data);
        let _version = r.u16()?;
        let _vendor = r.u32()?;
        let _vendor_version = r.u16()?;
        let _vendor_desc = r.string()?;
        let _mode = r.u16()?;
        let operations = r.array16()?;
        let _events = r.array16()?;
        let properties = r.array16()?;
        let _capture_formats = r.array16()?;
        let _image_formats = r.array16()?;
        Ok(Self {
            manufacturer: r.string()?,
            model: r.string()?,
            firmware: r.string()?,
            serial: r.string()?,
            operations,
            properties,
        })
    }

    /// Le boîtier signale ses changements par `FUJI_CHANGES`.
    pub fn reports_changes(&self) -> bool {
        self.properties.contains(&(FUJI_CHANGES as u16))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    pub storage: u32,
    pub format: u16,
    /// Taille en octets ; `None` au-delà de 4 Go.
    pub size: Option<u64>,
    pub name: String,
    /// Date de prise de vue telle que donnée par le boîtier (`20261031T142500`).
    pub captured: String,
}

impl ObjectInfo {
    /// Format « association » : un dossier.
    pub const FOLDER: u16 = 0x3001;

    pub fn parse(data: &[u8]) -> Result<Self, TetherError> {
        let mut r = Reader::new(data);
        let storage = r.u32()?;
        let format = r.u16()?;
        let _protection = r.u16()?;
        let size = r.u32()?;
        // Vignette, dimensions, parent, association, séquence (ISO 15740).
        r.skip(40)?;
        let name = r.string()?;
        let captured = r.string()?;
        Ok(Self {
            storage,
            format,
            size: (size != SIZE_UNKNOWN).then_some(u64::from(size)),
            name,
            captured,
        })
    }
}

/// Destination qui compte les octets reçus.
struct Counted<W> {
    inner: W,
    count: u64,
}

impl<W: Write> Write for Counted<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.count += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Boîtier relié, vu à travers son transport.
pub struct Camera<T> {
    transport: T,
}

impl<T: Transport> Camera<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    fn call(&self, op: u16, params: &[u32], sink: &mut dyn Write) -> Result<(), TetherError> {
        let code = self.transport.read(Operation::new(op)?, params, sink)?;
        if code == RESPONSE_OK {
            Ok(())
        } else {
            Err(TetherError::Response { op, code })
        }
    }

    fn data(&self, op: u16, params: &[u32]) -> Result<Vec<u8>, TetherError> {
        let mut out = Vec::new();
        self.call(op, params, &mut out)?;
        Ok(out)
    }

    pub fn info(&self) -> Result<DeviceInfo, TetherError> {
        DeviceInfo::parse(&self.data(GET_DEVICE_INFO, &[])?)
    }

    pub fn storages(&self) -> Result<Vec<u32>, TetherError> {
        Reader::new(&self.data(GET_STORAGE_IDS, &[])?).array32()
    }

    /// Objets de toutes les mémoires.
    pub fn handles(&self) -> Result<Vec<u32>, TetherError> {
        Reader::new(&self.data(GET_OBJECT_HANDLES, &[0xFFFF_FFFF, 0, 0])?).array32()
    }

    pub fn object(&self, handle: u32) -> Result<ObjectInfo, TetherError> {
        ObjectInfo::parse(&self.data(GET_OBJECT_INFO, &[handle])?)
    }

    /// Propriétés changées depuis la dernière lecture (Fujifilm).
    pub fn changes(&self) -> Result<Vec<(u16, u32)>, TetherError> {
        let data = self.data(GET_DEVICE_PROP_VALUE, &[FUJI_CHANGES])?;
        let mut r = Reader::new(&data);
        let n = r.u16()?;
        (0..n).map(|_| Ok((r.u16()?, r.u32()?))).collect()
    }

    /// Photos que la carte peut encore recevoir (Fujifilm).
    pub fn remaining_shots(&self) -> Result<u32, TetherError> {
        let data = self.data(GET_DEVICE_PROP_VALUE, &[FUJI_REMAINING_SHOTS])?;
        let mut r = Reader::new(&data);
        match data.len() {
            2 | 3 => r.u16().map(u32::from),
            _ => r.u32(),
        }
    }

    /// Lit un objet dans `target`, en passant par un fichier `.afterdusk-part`
    /// renommé une fois la taille annoncée atteinte ; rend la taille reçue.
    pub fn download(
        &self,
        handle: u32,
        info: &ObjectInfo,
        target: &Path,
    ) -> Result<u64, TetherError> {
        let part = part_path(target);
        let result = (|| {
            let mut sink = Counted {
                inner: BufWriter::new(File::create(&part)?),
                count: 0,
            };
            self.call(GET_OBJECT, &[handle], &mut sink)?;
            sink.flush()?;
            let got = sink.count;
            if let Some(expected) = info.size
                && got != expected
            {
                return Err(TetherError::Incomplete { expected, got });
            }
            let file = sink
                .inner
                .into_inner()
                .map_err(|e| TetherError::Io(e.into_error()))?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&part, target)?;
            Ok(got)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seules_les_lectures_passent() {
        // Écritures et suppressions d'ISO 15740 : jamais envoyées.
        for code in [
            0x100B_u16, // DeleteObject
            0x100C,     // SendObjectInfo
            0x100D,     // SendObject
            0x100E,     // InitiateCapture
            0x100F,     // FormatStore
            0x1010,     // ResetDevice
            0x1012,     // SetObjectProtection
            0x1016,     // SetDevicePropValue
            0x1017,     // ResetDevicePropValue
            0x1019,     // MoveObject
            0x101A,     // CopyObject
        ] {
            assert!(matches!(
                Operation::new(code),
                Err(TetherError::Refused(c)) if c == code
            ));
        }
        let allowed: Vec<u16> = (0..=u16::MAX)
            .filter(|c| Operation::new(*c).is_ok())
            .collect();
        assert_eq!(
            allowed,
            vec![0x1001, 0x1004, 0x1007, 0x1008, 0x1009, 0x1015]
        );
    }

    #[test]
    fn donnees_tronquees_refusees() {
        assert!(matches!(
            ObjectInfo::parse(&[1, 0, 0]),
            Err(TetherError::Malformed(_))
        ));
        assert!(matches!(
            DeviceInfo::parse(&[]),
            Err(TetherError::Malformed(_))
        ));
    }
}
