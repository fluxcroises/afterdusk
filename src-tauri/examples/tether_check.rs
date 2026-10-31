//! Essai du transport PTP de Windows avec un vrai boîtier (V0.8, étape 2a) :
//! fiche d'identité, place restante, puis lecture de la dernière photo de la
//! carte dans `dossier`. Rien n'est écrit sur le boîtier.
//!
//!   cargo run --example tether_check -- <dossier>

#[cfg(not(windows))]
fn main() {
    eprintln!("essai réservé à Windows (Windows Portable Devices)");
}

#[cfg(windows)]
fn main() -> Result<(), afterdusk_lib::tether::TetherError> {
    use std::time::Instant;

    use afterdusk_lib::tether::ptp::Camera;
    use afterdusk_lib::tether::wpd::{WpdTransport, devices};

    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let list = devices()?;
    for d in &list {
        println!("appareil : {} ({}) {}", d.name, d.manufacturer, d.id);
    }
    let Some(found) = list
        .iter()
        .find(|d| d.manufacturer.eq_ignore_ascii_case("fujifilm"))
    else {
        println!("aucun boîtier Fujifilm relié");
        return Ok(());
    };
    let camera = Camera::new(WpdTransport::open(&found.id)?);
    let info = camera.info()?;
    println!(
        "{} {}, micrologiciel {}, changements signalés : {}",
        info.manufacturer,
        info.model,
        info.firmware,
        info.reports_changes()
    );
    println!("place restante : {} photos", camera.remaining_shots()?);
    println!("changements en attente : {:?}", camera.changes()?);
    let handles = camera.handles()?;
    println!("{} objets sur la carte", handles.len());
    let Some(&last) = handles.last() else {
        return Ok(());
    };
    let object = camera.object(last)?;
    println!("dernier : {object:?}");
    let target = dir.join(&object.name);
    let started = Instant::now();
    let size = camera.download(last, &object, &target)?;
    let secs = started.elapsed().as_secs_f64();
    println!(
        "lu : {} ({size} octets) en {secs:.2} s, {:.0} Mo/s",
        target.display(),
        size as f64 / secs / 1e6
    );
    Ok(())
}
