//! Transport PTP de Windows : les « commandes MTP étendues » de Windows
//! Portable Devices, sans changer de pilote (décision 0016).
//!
//! Windows n'accepte ces commandes que d'un programme qui a ouvert l'appareil
//! avec droit d'écriture, même pour lire (accord de l'utilisateur,
//! 2026-10-31) ; ce droit ne sert à rien d'autre. La liste des appareils se
//! lit sans en ouvrir aucun.

use std::io::Write;

use windows::Win32::Devices::PortableDevices::*;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Variant::VT_UI4;
use windows::core::{HRESULT, PCWSTR, PWSTR, w};

use super::TetherError;
use super::ptp::{Operation, Transport};

/// GENERIC_READ | GENERIC_WRITE : exigé par Windows pour toute commande PTP.
const READ_WRITE: u32 = 0xC000_0000;
/// Morceau lu à chaque appel pendant un transfert.
const CHUNK: u32 = 1024 * 1024;

/// Codes Windows d'un appareil parti.
const GONE: [u32; 4] = [
    0x8007_048F, // ERROR_DEVICE_NOT_CONNECTED
    0x8007_001F, // ERROR_GEN_FAILURE
    0x8007_0015, // ERROR_NOT_READY
    0x8007_01B1, // ERROR_NO_SUCH_DEVICE
];

impl From<windows::core::Error> for TetherError {
    fn from(e: windows::core::Error) -> Self {
        if GONE.contains(&(e.code().0 as u32)) {
            TetherError::Disconnected
        } else {
            TetherError::Device(e.message())
        }
    }
}

/// COM, une fois par fil ; un fil déjà initialisé autrement convient aussi.
fn com() {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

/// Chaîne allouée par COM, rendue en `String` et libérée.
fn take(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { p.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Appareil USB vu par Windows Portable Devices, sans l'ouvrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
}

type DeviceString =
    unsafe fn(&IPortableDeviceManager, PCWSTR, PWSTR, *mut u32) -> windows::core::Result<()>;

fn device_string(manager: &IPortableDeviceManager, id: &[u16], get: DeviceString) -> String {
    let mut len = 0u32;
    let _ = unsafe { get(manager, PCWSTR(id.as_ptr()), PWSTR::null(), &mut len) };
    if len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize];
    match unsafe {
        get(
            manager,
            PCWSTR(id.as_ptr()),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    } {
        Ok(()) => String::from_utf16_lossy(&buf)
            .trim_end_matches('\0')
            .to_owned(),
        Err(_) => String::new(),
    }
}

/// Appareils USB branchés (les lecteurs de cartes n'en sont pas).
pub fn devices() -> Result<Vec<Detected>, TetherError> {
    com();
    let manager: IPortableDeviceManager =
        unsafe { CoCreateInstance(&PortableDeviceManager, None, CLSCTX_INPROC_SERVER)? };
    unsafe { manager.RefreshDeviceList()? };
    let mut count = 0u32;
    unsafe { manager.GetDevices(std::ptr::null_mut(), &mut count)? };
    let mut ids = vec![PWSTR::null(); count as usize];
    if count > 0 {
        unsafe { manager.GetDevices(ids.as_mut_ptr(), &mut count)? };
    }
    let mut out = Vec::new();
    for id in ids.into_iter().take(count as usize) {
        let id = take(id);
        if !id.to_lowercase().contains("usb#vid_") {
            continue;
        }
        let w = wide(&id);
        out.push(Detected {
            name: device_string(&manager, &w, |m, a, b, c| unsafe {
                m.GetDeviceFriendlyName(a, b, c)
            }),
            manufacturer: device_string(&manager, &w, |m, a, b, c| unsafe {
                m.GetDeviceManufacturer(a, b, c)
            }),
            id,
        });
    }
    Ok(out)
}

/// Boîtier ouvert.
pub struct WpdTransport {
    device: IPortableDevice,
}

impl Drop for WpdTransport {
    fn drop(&mut self) {
        let _ = unsafe { self.device.Close() };
    }
}

fn values() -> windows::core::Result<IPortableDeviceValues> {
    unsafe { CoCreateInstance(&PortableDeviceValues, None, CLSCTX_INPROC_SERVER) }
}

fn command(key: &PROPERTYKEY) -> windows::core::Result<IPortableDeviceValues> {
    let v = values()?;
    unsafe {
        v.SetGuidValue(&WPD_PROPERTY_COMMON_COMMAND_CATEGORY, &key.fmtid)?;
        v.SetUnsignedIntegerValue(&WPD_PROPERTY_COMMON_COMMAND_ID, key.pid)?;
    }
    Ok(v)
}

impl WpdTransport {
    pub fn open(id: &str) -> Result<Self, TetherError> {
        com();
        let id = wide(id);
        let client = values()?;
        let device: IPortableDevice =
            unsafe { CoCreateInstance(&PortableDeviceFTM, None, CLSCTX_INPROC_SERVER)? };
        unsafe {
            client.SetStringValue(&WPD_CLIENT_NAME, w!("Afterdusk"))?;
            client.SetUnsignedIntegerValue(&WPD_CLIENT_DESIRED_ACCESS, READ_WRITE)?;
            device.Open(PCWSTR(id.as_ptr()), &client)?;
        }
        Ok(Self { device })
    }

    fn send(&self, v: &IPortableDeviceValues) -> windows::core::Result<IPortableDeviceValues> {
        let r = unsafe { self.device.SendCommand(0, v)? };
        if let Ok(hr) = unsafe { r.GetErrorValue(&WPD_PROPERTY_COMMON_HRESULT) } {
            HRESULT(hr.0).ok()?;
        }
        Ok(r)
    }

    fn start(&self, op: Operation, params: &[u32]) -> windows::core::Result<(Vec<u16>, u64)> {
        let v = command(&WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ)?;
        unsafe {
            v.SetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_OPERATION_CODE, u32::from(op.code()))?;
            let list: IPortableDevicePropVariantCollection = CoCreateInstance(
                &PortableDevicePropVariantCollection,
                None,
                CLSCTX_INPROC_SERVER,
            )?;
            for p in params {
                let mut var = PROPVARIANT::default();
                (*var.Anonymous.Anonymous).vt = VT_UI4;
                (*var.Anonymous.Anonymous).Anonymous.ulVal = *p;
                list.Add(&var)?;
                PropVariantClear(&mut var)?;
            }
            v.SetIPortableDevicePropVariantCollectionValue(
                &WPD_PROPERTY_MTP_EXT_OPERATION_PARAMS,
                &list,
            )?;
        }
        let r = self.send(&v)?;
        let context = take(unsafe { r.GetStringValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT)? });
        let total = unsafe {
            r.GetUnsignedLargeIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE)
        }
        .or_else(|_| {
            unsafe { r.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE) }
                .map(u64::from)
        })
        .unwrap_or(0);
        Ok((wide(&context), total))
    }

    /// Un morceau du transfert ; vide à la fin.
    fn chunk(&self, context: &[u16], want: u32) -> windows::core::Result<Vec<u8>> {
        let c = command(&WPD_COMMAND_MTP_EXT_READ_DATA)?;
        unsafe {
            c.SetStringValue(
                &WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT,
                PCWSTR(context.as_ptr()),
            )?;
            c.SetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ, want)?;
            c.SetBufferValue(
                &WPD_PROPERTY_MTP_EXT_TRANSFER_DATA,
                &vec![0u8; want as usize],
            )?;
        }
        let r = self.send(&c)?;
        let mut p = std::ptr::null_mut();
        let mut len = 0u32;
        unsafe { r.GetBufferValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_DATA, &mut p, &mut len)? };
        let got =
            unsafe { r.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ) }
                .unwrap_or(len)
                .min(len);
        let mut out = Vec::new();
        if !p.is_null() {
            out.extend_from_slice(unsafe { std::slice::from_raw_parts(p, got as usize) });
            unsafe { CoTaskMemFree(Some(p as *const _)) };
        }
        Ok(out)
    }

    /// Clôt le transfert ; rend le code de réponse PTP.
    fn end(&self, context: &[u16]) -> windows::core::Result<u16> {
        let e = command(&WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER)?;
        unsafe {
            e.SetStringValue(
                &WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT,
                PCWSTR(context.as_ptr()),
            )?
        };
        let r = self.send(&e)?;
        let code =
            unsafe { r.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_RESPONSE_CODE) }.unwrap_or(0);
        Ok(code as u16)
    }
}

impl Transport for WpdTransport {
    fn read(
        &self,
        op: Operation,
        params: &[u32],
        sink: &mut dyn Write,
    ) -> Result<u16, TetherError> {
        let (context, total) = self.start(op, params)?;
        let mut done = 0u64;
        let transfer = (|| {
            while done < total {
                let want = (total - done).min(u64::from(CHUNK)) as u32;
                let data = self.chunk(&context, want)?;
                if data.is_empty() {
                    break;
                }
                sink.write_all(&data)?;
                done += data.len() as u64;
            }
            Ok::<(), TetherError>(())
        })();
        // Le transfert est clos même interrompu, sinon le boîtier reste occupé.
        let code = self.end(&context);
        transfer?;
        Ok(code?)
    }
}
