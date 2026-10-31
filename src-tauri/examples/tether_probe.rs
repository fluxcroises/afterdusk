//! Sonde du tethering (V0.8, étape 1) : ce que Windows Portable Devices voit
//! des appareils branchés (modèle, protocole, contenu), puis, pendant quelques
//! minutes, les annonces du boîtier et les fichiers qui y apparaissent ou en
//! disparaissent. Lecture seule : le boîtier est ouvert sans droit
//! d'écriture, rien n'y est écrit ni supprimé.
//!
//!   cargo run --example tether_probe -- [minutes, 3 par défaut]

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
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    use windows::Win32::Devices::PortableDevices::*;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    };
    use windows::core::{GUID, PCWSTR, PWSTR, Ref, Result, implement, w};

    /// Droit de lecture seul (GENERIC_READ).
    const READ_ONLY: u32 = 0x8000_0000;
    /// Objets au plus parcourus par appareil (une carte pleine en a des milliers).
    const OBJECTS_MAX: usize = 20_000;
    /// Écart entre deux relevés du contenu.
    const POLL: Duration = Duration::from_secs(3);

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

    fn string(v: &IPortableDeviceValues, key: &PROPERTYKEY) -> Option<String> {
        unsafe { v.GetStringValue(key) }.ok().map(take)
    }

    fn event_name(id: &GUID) -> String {
        let known = [
            (WPD_EVENT_OBJECT_ADDED, "objet ajouté"),
            (WPD_EVENT_OBJECT_REMOVED, "objet retiré"),
            (WPD_EVENT_OBJECT_UPDATED, "objet modifié"),
            (WPD_EVENT_OBJECT_TRANSFER_REQUESTED, "transfert demandé"),
            (WPD_EVENT_DEVICE_REMOVED, "appareil débranché"),
        ];
        known
            .iter()
            .find(|(g, _)| g == id)
            .map_or_else(|| format!("{id:?}"), |(_, n)| (*n).to_owned())
    }

    /// Annonces du boîtier, affichées à leur arrivée.
    #[implement(IPortableDeviceEventCallback)]
    struct Events {
        device: String,
        started: Instant,
    }

    impl IPortableDeviceEventCallback_Impl for Events_Impl {
        fn OnEvent(&self, params: Ref<IPortableDeviceValues>) -> Result<()> {
            if let Some(v) = params.as_ref() {
                let id = unsafe { v.GetGuidValue(&WPD_EVENT_PARAMETER_EVENT_ID) }
                    .map_or_else(|_| "?".to_owned(), |g| event_name(&g));
                let name = string(v, &WPD_OBJECT_ORIGINAL_FILE_NAME)
                    .or_else(|| string(v, &WPD_OBJECT_NAME))
                    .unwrap_or_default();
                let object = string(v, &WPD_OBJECT_ID).unwrap_or_default();
                println!(
                    "[{:>6.1} s] annonce de {} : {id} {name} ({object})",
                    self.started.elapsed().as_secs_f64(),
                    self.device
                );
            }
            Ok(())
        }
    }

    /// Ouvre un appareil en lecture seule.
    fn open(pnp: &[u16]) -> Result<IPortableDevice> {
        unsafe {
            let client: IPortableDeviceValues =
                CoCreateInstance(&PortableDeviceValues, None, CLSCTX_INPROC_SERVER)?;
            client.SetStringValue(&WPD_CLIENT_NAME, w!("Afterdusk, sonde"))?;
            client.SetUnsignedIntegerValue(&WPD_CLIENT_DESIRED_ACCESS, READ_ONLY)?;
            let device: IPortableDevice =
                CoCreateInstance(&PortableDeviceFTM, None, CLSCTX_INPROC_SERVER)?;
            device.Open(PCWSTR(pnp.as_ptr()), &client)?;
            Ok(device)
        }
    }

    /// Identifiants de tous les objets sous `parent`, en profondeur.
    fn objects(content: &IPortableDeviceContent, parent: &str, out: &mut Vec<String>) {
        if out.len() >= OBJECTS_MAX {
            return;
        }
        let id = wide(parent);
        let Ok(list) = (unsafe { content.EnumObjects(0, PCWSTR(id.as_ptr()), None) }) else {
            return;
        };
        let mut children = Vec::new();
        loop {
            let mut batch = [PWSTR::null(); 64];
            let mut fetched = 0u32;
            let _ = unsafe { list.Next(&mut batch, &mut fetched) };
            if fetched == 0 {
                break;
            }
            children.extend(batch[..fetched as usize].iter().map(|p| take(*p)));
        }
        for child in children {
            out.push(child.clone());
            objects(content, &child, out);
        }
    }

    /// Nom, taille et type d'un objet.
    fn describe(content: &IPortableDeviceContent, object: &str) -> String {
        let id = wide(object);
        let values = unsafe { content.Properties() }
            .and_then(|p| unsafe { p.GetValues(PCWSTR(id.as_ptr()), None) });
        let Ok(v) = values else {
            return format!("{object} (propriétés illisibles)");
        };
        let name = string(&v, &WPD_OBJECT_ORIGINAL_FILE_NAME)
            .or_else(|| string(&v, &WPD_OBJECT_NAME))
            .unwrap_or_default();
        let size = unsafe { v.GetUnsignedLargeIntegerValue(&WPD_OBJECT_SIZE) }
            .map_or_else(|_| String::new(), |s| format!(", {} Ko", s / 1024));
        let kind = match unsafe { v.GetGuidValue(&WPD_OBJECT_CONTENT_TYPE) } {
            Ok(g) if g == WPD_CONTENT_TYPE_FOLDER => ", dossier",
            Ok(g) if g == WPD_CONTENT_TYPE_FUNCTIONAL_OBJECT => ", objet fonctionnel",
            _ => "",
        };
        format!("{name}{size}{kind} [{object}]")
    }

    struct Device {
        name: String,
        /// Gardé ouvert : les annonces arrivent tant qu'il l'est.
        _device: IPortableDevice,
        content: IPortableDeviceContent,
        known: BTreeMap<String, String>,
    }

    pub fn run() -> Result<()> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
        let minutes: u64 = std::env::args()
            .nth(1)
            .and_then(|a| a.parse().ok())
            .unwrap_or(3);
        let started = Instant::now();
        let manager: IPortableDeviceManager =
            unsafe { CoCreateInstance(&PortableDeviceManager, None, CLSCTX_INPROC_SERVER)? };
        let mut count = 0u32;
        unsafe { manager.GetDevices(std::ptr::null_mut(), &mut count)? };
        let mut ids = vec![PWSTR::null(); count as usize];
        if count > 0 {
            unsafe { manager.GetDevices(ids.as_mut_ptr(), &mut count)? };
        }
        println!("{count} appareil(s) vu(s) par Windows Portable Devices");

        let mut devices = Vec::new();
        for id in ids.into_iter().take(count as usize) {
            let pnp = take(id);
            let pnp_w = wide(&pnp);
            let mut len = 0u32;
            let _ = unsafe {
                manager.GetDeviceFriendlyName(PCWSTR(pnp_w.as_ptr()), PWSTR::null(), &mut len)
            };
            let mut buf = vec![0u16; len.max(1) as usize];
            let name = match unsafe {
                manager.GetDeviceFriendlyName(
                    PCWSTR(pnp_w.as_ptr()),
                    PWSTR(buf.as_mut_ptr()),
                    &mut len,
                )
            } {
                Ok(()) => String::from_utf16_lossy(&buf)
                    .trim_end_matches('\0')
                    .to_owned(),
                Err(_) => "(sans nom)".to_owned(),
            };
            println!("\n== {name}\n   {pnp}");
            let device = match open(&pnp_w) {
                Ok(d) => d,
                Err(e) => {
                    println!("   ouverture impossible : {e}");
                    continue;
                }
            };
            let content = unsafe { device.Content()? };
            if let Ok(v) = unsafe { content.Properties() }
                .and_then(|p| unsafe { p.GetValues(WPD_DEVICE_OBJECT_ID, None) })
            {
                for (label, key) in [
                    ("fabricant", WPD_DEVICE_MANUFACTURER),
                    ("modèle", WPD_DEVICE_MODEL),
                    ("protocole", WPD_DEVICE_PROTOCOL),
                ] {
                    println!("   {label} : {}", string(&v, &key).unwrap_or_default());
                }
            }
            let mut list = Vec::new();
            objects(&content, "DEVICE", &mut list);
            println!("   {} objet(s) ; les 15 premiers :", list.len());
            for object in list.iter().take(15) {
                println!("     {}", describe(&content, object));
            }
            let callback: IPortableDeviceEventCallback = Events {
                device: name.clone(),
                started,
            }
            .into();
            match unsafe { device.Advise(0, &callback, None) } {
                Ok(cookie) => drop(take(cookie)),
                Err(e) => println!("   annonces refusées : {e}"),
            }
            let known = list.into_iter().map(|o| (o, String::new())).collect();
            devices.push(Device {
                name,
                _device: device,
                content,
                known,
            });
        }

        println!(
            "\nÉcoute pendant {minutes} min : déclenche quelques photos (JPEG, RAW), \
             puis débranche et rebranche le câble une fois."
        );
        let deadline = started + Duration::from_secs(minutes * 60);
        while Instant::now() < deadline {
            std::thread::sleep(POLL);
            for d in &mut devices {
                let mut now = Vec::new();
                objects(&d.content, "DEVICE", &mut now);
                let t = started.elapsed().as_secs_f64();
                for object in &now {
                    if !d.known.contains_key(object) {
                        let desc = describe(&d.content, object);
                        println!("[{t:>6.1} s] nouveau sur {} : {desc}", d.name);
                        d.known.insert(object.clone(), desc);
                    }
                }
                let gone: Vec<String> = d
                    .known
                    .keys()
                    .filter(|k| !now.contains(k))
                    .cloned()
                    .collect();
                for object in gone {
                    println!("[{t:>6.1} s] disparu de {} : {object}", d.name);
                    d.known.remove(&object);
                }
            }
        }
        println!("\nFin de l'écoute.");
        Ok(())
    }
}
