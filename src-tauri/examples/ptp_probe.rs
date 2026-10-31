//! Seconde sonde du tethering (V0.8, étape 1) : le protocole de l'appareil
//! photo (PTP) parlé à travers Windows Portable Devices (« commandes MTP
//! étendues »), sans changer de pilote. Elle ne demande au boîtier que des
//! opérations de lecture : sa fiche d'identité (opérations, annonces et
//! propriétés qu'il connaît), ses mémoires et ses objets, la valeur de
//! propriétés. Aucune opération qui écrive ou supprime n'est envoyée.
//!
//! Puis, pendant quelques minutes, elle relève toutes les demi-secondes les
//! objets du boîtier et la propriété 0xD212 que Fujifilm utilise en prise de
//! vue reliée pour signaler ses changements (d'après libgphoto2).
//!
//!   cargo run --example ptp_probe -- [minutes, 3 par défaut]

#[cfg(not(windows))]
fn main() {
    eprintln!("sonde réservée à Windows (Windows Portable Devices)");
}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    probe::run()
}

#[cfg(windows)]
mod probe {
    use std::time::{Duration, Instant};

    use windows::Win32::Devices::PortableDevices::*;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    };
    use windows::Win32::System::Variant::VT_UI4;
    use windows::core::{PCWSTR, PWSTR, Result, w};

    const READ_ONLY: u32 = 0x8000_0000;
    const CHUNK: u32 = 256 * 1024;

    // Opérations PTP de lecture (ISO 15740).
    const GET_DEVICE_INFO: u16 = 0x1001;
    const GET_STORAGE_IDS: u16 = 0x1004;
    const GET_OBJECT_HANDLES: u16 = 0x1007;
    const GET_OBJECT_INFO: u16 = 0x1008;
    const GET_DEVICE_PROP_VALUE: u16 = 0x1015;
    /// Propriété Fujifilm des changements en prise de vue reliée.
    const FUJI_EVENTS: u32 = 0xD212;

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

    /// Lecteur de données PTP (petit-boutiste).
    struct Reader<'a> {
        data: &'a [u8],
        at: usize,
    }

    impl Reader<'_> {
        fn u8(&mut self) -> u8 {
            let v = self.data.get(self.at).copied().unwrap_or(0);
            self.at += 1;
            v
        }
        fn u16(&mut self) -> u16 {
            u16::from(self.u8()) | (u16::from(self.u8()) << 8)
        }
        fn u32(&mut self) -> u32 {
            u32::from(self.u16()) | (u32::from(self.u16()) << 16)
        }
        fn string(&mut self) -> String {
            let n = self.u8() as usize;
            let chars: Vec<u16> = (0..n).map(|_| self.u16()).collect();
            String::from_utf16_lossy(&chars)
                .trim_end_matches('\0')
                .to_owned()
        }
        fn array16(&mut self) -> Vec<u16> {
            let n = self.u32().min(4096);
            (0..n).map(|_| self.u16()).collect()
        }
    }

    fn hex(list: &[u16]) -> String {
        list.iter()
            .map(|v| format!("{v:04X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    struct Ptp {
        device: IPortableDevice,
    }

    impl Ptp {
        fn values() -> Result<IPortableDeviceValues> {
            unsafe { CoCreateInstance(&PortableDeviceValues, None, CLSCTX_INPROC_SERVER) }
        }

        fn command(key: &PROPERTYKEY) -> Result<IPortableDeviceValues> {
            let v = Self::values()?;
            unsafe {
                v.SetGuidValue(&WPD_PROPERTY_COMMON_COMMAND_CATEGORY, &key.fmtid)?;
                v.SetUnsignedIntegerValue(&WPD_PROPERTY_COMMON_COMMAND_ID, key.pid)?;
            }
            Ok(v)
        }

        fn send(&self, v: &IPortableDeviceValues) -> Result<IPortableDeviceValues> {
            let r = unsafe { self.device.SendCommand(0, v)? };
            if let Ok(hr) = unsafe { r.GetErrorValue(&WPD_PROPERTY_COMMON_HRESULT) } {
                hr.ok()?;
            }
            Ok(r)
        }

        fn operation(key: &PROPERTYKEY, op: u16, params: &[u32]) -> Result<IPortableDeviceValues> {
            let v = Self::command(key)?;
            unsafe {
                v.SetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_OPERATION_CODE, u32::from(op))?;
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
            Ok(v)
        }

        /// Opération qui renvoie des données : code de réponse et données.
        fn read(&self, op: u16, params: &[u32]) -> Result<(u32, Vec<u8>)> {
            let v = Self::operation(
                &WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ,
                op,
                params,
            )?;
            let r = self.send(&v)?;
            let context =
                take(unsafe { r.GetStringValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT)? });
            let total = unsafe {
                r.GetUnsignedLargeIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE)
            }
            .or_else(|_| {
                unsafe { r.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE) }
                    .map(u64::from)
            })
            .unwrap_or(0);
            let ctx = wide(&context);
            let mut data = Vec::new();
            while (data.len() as u64) < total {
                let want = (total - data.len() as u64).min(u64::from(CHUNK)) as u32;
                let c = Self::command(&WPD_COMMAND_MTP_EXT_READ_DATA)?;
                unsafe {
                    c.SetStringValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, PCWSTR(ctx.as_ptr()))?;
                    c.SetUnsignedIntegerValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ,
                        want,
                    )?;
                    c.SetBufferValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_DATA,
                        &vec![0u8; want as usize],
                    )?;
                }
                let r = self.send(&c)?;
                let mut p = std::ptr::null_mut();
                let mut len = 0u32;
                unsafe { r.GetBufferValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_DATA, &mut p, &mut len)? };
                let got = unsafe {
                    r.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ)
                }
                .unwrap_or(len)
                .min(len);
                if !p.is_null() {
                    data.extend_from_slice(unsafe { std::slice::from_raw_parts(p, got as usize) });
                    unsafe { CoTaskMemFree(Some(p as *const _)) };
                }
                if got == 0 {
                    break;
                }
            }
            let e = Self::command(&WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER)?;
            unsafe {
                e.SetStringValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, PCWSTR(ctx.as_ptr()))?
            };
            let r = self.send(&e)?;
            let code = unsafe { r.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_RESPONSE_CODE) }
                .unwrap_or(0);
            Ok((code, data))
        }
    }

    fn open() -> Result<Option<(String, IPortableDevice)>> {
        let manager: IPortableDeviceManager =
            unsafe { CoCreateInstance(&PortableDeviceManager, None, CLSCTX_INPROC_SERVER)? };
        let mut count = 0u32;
        unsafe { manager.GetDevices(std::ptr::null_mut(), &mut count)? };
        let mut ids = vec![PWSTR::null(); count as usize];
        if count > 0 {
            unsafe { manager.GetDevices(ids.as_mut_ptr(), &mut count)? };
        }
        for id in ids.into_iter().take(count as usize) {
            let pnp = take(id);
            // Le boîtier : un appareil USB (pas un lecteur de cartes).
            if !pnp.to_lowercase().contains("usb#vid_") {
                continue;
            }
            let pnp_w = wide(&pnp);
            let client = Ptp::values()?;
            let device: IPortableDevice =
                unsafe { CoCreateInstance(&PortableDeviceFTM, None, CLSCTX_INPROC_SERVER)? };
            unsafe {
                client.SetStringValue(&WPD_CLIENT_NAME, w!("Afterdusk, sonde PTP"))?;
                client.SetUnsignedIntegerValue(&WPD_CLIENT_DESIRED_ACCESS, READ_ONLY)?;
                device.Open(PCWSTR(pnp_w.as_ptr()), &client)?;
            }
            return Ok(Some((pnp, device)));
        }
        Ok(None)
    }

    pub fn run() -> Result<()> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
        let minutes: u64 = std::env::args()
            .nth(1)
            .and_then(|a| a.parse().ok())
            .unwrap_or(3);
        let Some((pnp, device)) = open()? else {
            println!("aucun boîtier USB vu par Windows Portable Devices");
            return Ok(());
        };
        println!("boîtier : {pnp}");
        let ptp = Ptp { device };

        let (code, info) = match ptp.read(GET_DEVICE_INFO, &[]) {
            Ok(r) => r,
            Err(e) => {
                println!("fiche d'identité PTP illisible : {e}");
                return Ok(());
            }
        };
        let mut r = Reader { data: &info, at: 0 };
        let version = r.u16();
        let vendor = r.u32();
        let vendor_version = r.u16();
        let vendor_desc = r.string();
        let mode = r.u16();
        let ops = r.array16();
        let events = r.array16();
        let props = r.array16();
        let capture = r.array16();
        let formats = r.array16();
        let (maker, model, firmware, serial) = (r.string(), r.string(), r.string(), r.string());
        println!("réponse {code:04X}, {} octets", info.len());
        println!(
            "PTP {version:04X}, extension {vendor:08X} v{vendor_version:04X} « {vendor_desc} », mode {mode:04X}"
        );
        println!(
            "{maker} {model}, micrologiciel {firmware}, n° {}",
            serial.len()
        );
        println!("opérations ({}) : {}", ops.len(), hex(&ops));
        println!("annonces ({}) : {}", events.len(), hex(&events));
        println!("propriétés ({}) : {}", props.len(), hex(&props));
        println!("formats de prise de vue : {}", hex(&capture));
        println!("formats d'image : {}", hex(&formats));

        match ptp.read(GET_STORAGE_IDS, &[]) {
            Ok((code, d)) => {
                let mut r = Reader { data: &d, at: 0 };
                let n = r.u32();
                let ids: Vec<String> = (0..n.min(16)).map(|_| format!("{:08X}", r.u32())).collect();
                println!("mémoires (réponse {code:04X}) : {}", ids.join(" "));
            }
            Err(e) => println!("mémoires illisibles : {e}"),
        }

        let handles = |ptp: &Ptp| -> Option<Vec<u32>> {
            let (_, d) = ptp.read(GET_OBJECT_HANDLES, &[0xFFFF_FFFF, 0, 0]).ok()?;
            let mut r = Reader { data: &d, at: 0 };
            let n = r.u32().min(100_000);
            Some((0..n).map(|_| r.u32()).collect())
        };
        let mut known = handles(&ptp).unwrap_or_default();
        println!("objets au départ : {}", known.len());
        let fuji = props.contains(&(FUJI_EVENTS as u16));
        println!(
            "propriété {FUJI_EVENTS:04X} connue du boîtier : {}",
            if fuji { "oui" } else { "non" }
        );

        println!(
            "\nÉcoute pendant {minutes} min : déclenche quelques photos (JPEG puis RAW), \
             sans débrancher."
        );
        let started = Instant::now();
        let deadline = started + Duration::from_secs(minutes * 60);
        let mut last_events = Vec::new();
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(500));
            let t = started.elapsed().as_secs_f64();
            if fuji {
                match ptp.read(GET_DEVICE_PROP_VALUE, &[FUJI_EVENTS]) {
                    Ok((_, d)) if d != last_events => {
                        let mut r = Reader { data: &d, at: 0 };
                        let n = r.u16().min(64);
                        let list: Vec<String> = (0..n)
                            .map(|_| {
                                let code = r.u16();
                                format!("{code:04X}={:08X}", r.u32())
                            })
                            .collect();
                        println!("[{t:>6.1} s] {FUJI_EVENTS:04X} : {}", list.join(" "));
                        last_events = d;
                    }
                    Ok(_) => {}
                    Err(e) => println!("[{t:>6.1} s] {FUJI_EVENTS:04X} illisible : {e}"),
                }
            }
            if let Some(now) = handles(&ptp) {
                for h in now.iter().filter(|h| !known.contains(h)) {
                    let name = ptp
                        .read(GET_OBJECT_INFO, &[*h])
                        .map(|(_, d)| {
                            let mut r = Reader { data: &d, at: 0 };
                            let _storage = r.u32();
                            let format = r.u16();
                            let _protection = r.u16();
                            let size = r.u32();
                            // Champs fixes jusqu'au nom (ISO 15740, ObjectInfo).
                            r.at = 52;
                            format!("{} ({format:04X}, {} Ko)", r.string(), size / 1024)
                        })
                        .unwrap_or_else(|e| format!("infos illisibles : {e}"));
                    println!("[{t:>6.1} s] nouvel objet {h:08X} : {name}");
                }
                for h in known.iter().filter(|h| !now.contains(h)) {
                    println!("[{t:>6.1} s] objet disparu {h:08X}");
                }
                known = now;
            }
        }
        println!("\nFin de l'écoute.");
        Ok(())
    }
}
