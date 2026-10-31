//! Boîtier factice qui imite le Fujifilm X-H2 en prise de vue reliée
//! (sondes de la décision 0016) : photos sur la carte, changements signalés
//! par 0xD212, place restante qui baisse, débranchement. Sert aux tests, les
//! machines d'intégration n'ayant pas de boîtier.

use std::io::Write;
use std::sync::Mutex;

use super::TetherError;
use super::ptp::{self, Operation, Transport};

const STORAGE: u32 = 0x1000_0001;
const FORMAT_RAF: u16 = 0xB103;
const FORMAT_JPEG: u16 = 0x3801;

struct Object {
    handle: u32,
    name: String,
    data: Vec<u8>,
}

struct State {
    objects: Vec<Object>,
    next_handle: u32,
    remaining: u32,
    changes: Vec<(u16, u32)>,
    connected: bool,
    cut_next: bool,
    sent: Vec<u16>,
}

pub struct FakeCamera {
    state: Mutex<State>,
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_string(out: &mut Vec<u8>, s: &str) {
    if s.is_empty() {
        out.push(0);
        return;
    }
    let chars: Vec<u16> = s.encode_utf16().chain(Some(0)).collect();
    out.push(chars.len() as u8);
    for c in chars {
        put_u16(out, c);
    }
}

fn put_array16(out: &mut Vec<u8>, list: &[u16]) {
    put_u32(out, list.len() as u32);
    for v in list {
        put_u16(out, *v);
    }
}

fn put_array32(out: &mut Vec<u8>, list: &[u32]) {
    put_u32(out, list.len() as u32);
    for v in list {
        put_u32(out, *v);
    }
}

impl FakeCamera {
    /// Boîtier dont la carte contient déjà `on_card` (noms de fichiers) et
    /// peut encore recevoir `remaining` photos.
    pub fn new(on_card: &[&str], remaining: u32) -> Self {
        let camera = Self {
            state: Mutex::new(State {
                objects: Vec::new(),
                next_handle: 1,
                remaining,
                changes: Vec::new(),
                connected: true,
                cut_next: false,
                sent: Vec::new(),
            }),
        };
        for name in on_card {
            camera.add(name, name.as_bytes().repeat(100));
        }
        camera.lock().changes.clear();
        camera
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn add(&self, name: &str, data: Vec<u8>) {
        let mut s = self.lock();
        let handle = s.next_handle;
        s.next_handle += 1;
        s.objects.push(Object {
            handle,
            name: name.to_owned(),
            data,
        });
    }

    /// Un déclenchement : la photo arrive sur la carte, la place baisse.
    pub fn shoot(&self, name: &str, data: Vec<u8>) {
        self.add(name, data);
        let mut s = self.lock();
        s.remaining = s.remaining.saturating_sub(1);
        let remaining = s.remaining;
        s.changes
            .push((ptp::FUJI_REMAINING_SHOTS as u16, remaining));
    }

    pub fn unplug(&self) {
        self.lock().connected = false;
    }

    /// Rebranché : comme le vrai boîtier, il renumérote ses objets.
    pub fn plug(&self) {
        let mut s = self.lock();
        s.connected = true;
        let base = s.next_handle + 100;
        for (i, o) in s.objects.iter_mut().enumerate() {
            o.handle = base + i as u32;
        }
        s.next_handle = base + s.objects.len() as u32;
    }

    /// Le prochain transfert d'objet s'interrompt à mi-chemin.
    pub fn cut_next_transfer(&self) {
        self.lock().cut_next = true;
    }

    /// Codes des opérations reçues, dans l'ordre.
    pub fn sent(&self) -> Vec<u16> {
        self.lock().sent.clone()
    }

    /// Noms et contenus des fichiers de la carte.
    pub fn card(&self) -> Vec<(String, Vec<u8>)> {
        self.lock()
            .objects
            .iter()
            .map(|o| (o.name.clone(), o.data.clone()))
            .collect()
    }
}

impl Transport for FakeCamera {
    fn read(
        &self,
        op: Operation,
        params: &[u32],
        sink: &mut dyn Write,
    ) -> Result<u16, TetherError> {
        let mut s = self.lock();
        s.sent.push(op.code());
        if !s.connected {
            return Err(TetherError::Disconnected);
        }
        let param = |i: usize| params.get(i).copied().unwrap_or(0);
        let mut out = Vec::new();
        match op.code() {
            ptp::GET_DEVICE_INFO => {
                put_u16(&mut out, 100);
                put_u32(&mut out, 6);
                put_u16(&mut out, 100);
                put_string(&mut out, "fujifilm.co.jp: 1.0; ");
                put_u16(&mut out, 0);
                put_array16(&mut out, &ptp::ALLOWED);
                put_array16(&mut out, &[]);
                put_array16(
                    &mut out,
                    &[ptp::FUJI_REMAINING_SHOTS as u16, ptp::FUJI_CHANGES as u16],
                );
                put_array16(&mut out, &[FORMAT_JPEG, FORMAT_RAF]);
                put_array16(&mut out, &[FORMAT_JPEG, FORMAT_RAF]);
                put_string(&mut out, "FUJIFILM");
                put_string(&mut out, "X-H2");
                put_string(&mut out, "5.30");
                put_string(&mut out, "FAKE0001");
            }
            ptp::GET_STORAGE_IDS => put_array32(&mut out, &[STORAGE]),
            ptp::GET_OBJECT_HANDLES => {
                let handles: Vec<u32> = s.objects.iter().map(|o| o.handle).collect();
                put_array32(&mut out, &handles);
            }
            ptp::GET_OBJECT_INFO => {
                let Some(o) = s.objects.iter().find(|o| o.handle == param(0)) else {
                    return Ok(0x2009); // Invalid_ObjectHandle
                };
                let raw = o.name.to_lowercase().ends_with(".raf");
                put_u32(&mut out, STORAGE);
                put_u16(&mut out, if raw { FORMAT_RAF } else { FORMAT_JPEG });
                put_u16(&mut out, 0);
                put_u32(&mut out, o.data.len() as u32);
                out.extend_from_slice(&[0; 40]);
                put_string(&mut out, &o.name);
                put_string(&mut out, "20261031T142500");
                put_string(&mut out, "");
                put_string(&mut out, "");
            }
            ptp::GET_OBJECT => {
                let cut = std::mem::take(&mut s.cut_next);
                let Some(o) = s.objects.iter().find(|o| o.handle == param(0)) else {
                    return Ok(0x2009);
                };
                if cut {
                    sink.write_all(&o.data[..o.data.len() / 2])?;
                    return Err(TetherError::Disconnected);
                }
                sink.write_all(&o.data)?;
                return Ok(ptp::RESPONSE_OK);
            }
            ptp::GET_DEVICE_PROP_VALUE => match param(0) {
                ptp::FUJI_CHANGES => {
                    let changes = std::mem::take(&mut s.changes);
                    put_u16(&mut out, changes.len() as u16);
                    for (code, value) in changes {
                        put_u16(&mut out, code);
                        put_u32(&mut out, value);
                    }
                }
                ptp::FUJI_REMAINING_SHOTS => put_u32(&mut out, s.remaining),
                _ => return Ok(0x200A), // DeviceProp_Not_Supported
            },
            _ => return Ok(0x2005), // Operation_Not_Supported
        }
        sink.write_all(&out)?;
        Ok(ptp::RESPONSE_OK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tether::ptp::Camera;

    #[test]
    fn lit_le_boitier_factice() {
        let camera = Camera::new(FakeCamera::new(&["DSCF0001.RAF", "DSCF0002.JPG"], 25));
        let info = camera.info().unwrap();
        assert_eq!(
            (info.manufacturer.as_str(), info.model.as_str()),
            ("FUJIFILM", "X-H2")
        );
        assert_eq!(info.serial, "FAKE0001");
        assert!(info.reports_changes());
        assert_eq!(camera.storages().unwrap(), vec![STORAGE]);
        let handles = camera.handles().unwrap();
        assert_eq!(handles.len(), 2);
        let first = camera.object(handles[0]).unwrap();
        assert_eq!(first.name, "DSCF0001.RAF");
        assert_eq!(first.format, FORMAT_RAF);
        assert_eq!(first.size, Some(1200));
        assert_eq!(first.captured, "20261031T142500");
        assert!(camera.changes().unwrap().is_empty());
        assert_eq!(camera.remaining_shots().unwrap(), 25);

        camera.transport().shoot("DSCF0003.RAF", vec![7; 5000]);
        assert_eq!(camera.changes().unwrap(), vec![(0xD20E, 24)]);
        // Lue une fois, la liste des changements se vide.
        assert!(camera.changes().unwrap().is_empty());
        assert_eq!(camera.handles().unwrap().len(), 3);
        assert!(matches!(
            camera.object(999),
            Err(TetherError::Response {
                op: 0x1008,
                code: 0x2009
            })
        ));
    }

    #[test]
    fn photo_recue_entiere_ou_pas_du_tout() {
        let dir = tempfile::tempdir().unwrap();
        let camera = Camera::new(FakeCamera::new(&[], 10));
        camera
            .transport()
            .shoot("DSCF0001.RAF", (0..=255).cycle().take(300_000).collect());
        let handle = camera.handles().unwrap()[0];
        let info = camera.object(handle).unwrap();

        // Coupure pendant le transfert : rien sous le nom final, pas de reste.
        camera.transport().cut_next_transfer();
        let target = dir.path().join("DSCF0001.RAF");
        assert!(camera.download(handle, &info, &target).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);

        let got = camera.download(handle, &info, &target).unwrap();
        assert_eq!(got, 300_000);
        assert_eq!(
            std::fs::read(&target).unwrap(),
            camera.transport().card()[0].1
        );

        // Taille annoncée différente de la taille reçue : refusée.
        let wrong = ptp::ObjectInfo {
            size: Some(1),
            ..info.clone()
        };
        let other = dir.path().join("autre.RAF");
        assert!(matches!(
            camera.download(handle, &wrong, &other),
            Err(TetherError::Incomplete {
                expected: 1,
                got: 300_000
            })
        ));
        assert!(!other.exists());
    }

    #[test]
    fn debranche_puis_rebranche() {
        let camera = Camera::new(FakeCamera::new(&["DSCF0001.JPG"], 10));
        let before = camera.handles().unwrap();
        camera.transport().unplug();
        assert!(matches!(camera.handles(), Err(TetherError::Disconnected)));
        camera.transport().plug();
        let after = camera.handles().unwrap();
        assert_eq!(after.len(), 1);
        assert_ne!(before, after);
        assert_eq!(camera.object(after[0]).unwrap().name, "DSCF0001.JPG");
    }

    #[test]
    fn rien_d_autre_que_des_lectures_n_atteint_le_boitier() {
        let dir = tempfile::tempdir().unwrap();
        let camera = Camera::new(FakeCamera::new(&["DSCF0001.RAF"], 3));
        camera.info().unwrap();
        camera.storages().unwrap();
        camera.transport().shoot("DSCF0002.JPG", vec![1; 10]);
        camera.changes().unwrap();
        camera.remaining_shots().unwrap();
        for h in camera.handles().unwrap() {
            let info = camera.object(h).unwrap();
            camera
                .download(h, &info, &dir.path().join(&info.name))
                .unwrap();
        }
        let sent = camera.transport().sent();
        assert!(sent.len() >= 9);
        assert!(sent.iter().all(|op| ptp::ALLOWED.contains(op)));
        // La carte n'a pas bougé.
        let card = camera.transport().card();
        assert_eq!(card.len(), 2);
        assert_eq!(card[0].1, "DSCF0001.RAF".as_bytes().repeat(100));
    }
}
