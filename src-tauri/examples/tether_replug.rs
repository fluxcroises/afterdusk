//! Essai du débranchement en cours de séance (V0.8, avant l'étape 2b) :
//! liste des objets du boîtier avant et après un débranchement, pour savoir
//! si les photos prises entre-temps reparaissent. Lecture seule, aucune photo
//! n'est lue.
//!
//!   cargo run --example tether_replug -- [minutes, 4 par défaut]

#[cfg(not(windows))]
fn main() {
    eprintln!("essai réservé à Windows (Windows Portable Devices)");
}

#[cfg(windows)]
fn main() {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    use afterdusk_lib::tether::ptp::Camera;
    use afterdusk_lib::tether::wpd::{WpdTransport, devices};

    let minutes: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(4);
    let started = Instant::now();
    let deadline = started + Duration::from_secs(minutes * 60);
    let t = || started.elapsed().as_secs_f64();
    println!(
        "Pendant {minutes} min : déclenche deux photos, débranche le câble, déclenche une \
         photo, rebranche, puis déclenche encore une photo."
    );
    let mut connection = 0;
    while Instant::now() < deadline {
        let found = devices().ok().and_then(|list| {
            list.into_iter()
                .find(|d| d.manufacturer.eq_ignore_ascii_case("fujifilm"))
        });
        let Some(found) = found else {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        };
        let camera = match WpdTransport::open(&found.id) {
            Ok(t) => Camera::new(t),
            Err(e) => {
                println!("[{:>6.1} s] ouverture impossible : {e}", t());
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };
        connection += 1;
        let place = camera
            .remaining_shots()
            .map_or_else(|e| e.to_string(), |n| n.to_string());
        println!(
            "[{:>6.1} s] connexion {connection}, place restante {place}",
            t()
        );
        let mut known: BTreeMap<u32, String> = BTreeMap::new();
        loop {
            if Instant::now() >= deadline {
                break;
            }
            match camera.handles() {
                Ok(handles) => {
                    let fresh: Vec<u32> = handles
                        .iter()
                        .filter(|h| !known.contains_key(h))
                        .copied()
                        .collect();
                    for h in &fresh {
                        let desc = camera.object(*h).map_or_else(
                            |e| format!("fiche illisible : {e}"),
                            |o| format!("{} {:?} octets, pris {}", o.name, o.size, o.captured),
                        );
                        println!("[{:>6.1} s]   objet {h:08X} : {desc}", t());
                        known.insert(*h, desc);
                    }
                    for h in known.keys().filter(|h| !handles.contains(h)) {
                        println!("[{:>6.1} s]   objet disparu {h:08X}", t());
                    }
                    known.retain(|h, _| handles.contains(h));
                }
                Err(e) => {
                    println!("[{:>6.1} s] connexion {connection} perdue : {e}", t());
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    println!("\nFin de l'essai.");
}
