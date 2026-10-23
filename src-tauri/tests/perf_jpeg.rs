//! Ouverture d'un grand JPEG de boîtier dans le Retoucher : décodage, passage
//! en XYZ linéaire et image de travail.
//! La photo n'est pas dans test-assets (40 Mpx, droits de l'utilisateur) :
//! `AFTERDUSK_JPEG=<chemin> cargo test --release --test perf_jpeg -- --ignored --nocapture`

use std::path::PathBuf;
use std::time::Instant;

use afterdusk_lib::develop::pipeline::WorkingImage;
use afterdusk_lib::develop::rendered;
use afterdusk_lib::develop::source::PREVIEW_SIDE;

#[test]
#[ignore = "mesure, sur un JPEG donné par AFTERDUSK_JPEG"]
fn image_de_travail_d_un_grand_jpeg() {
    let Some(path) = std::env::var_os("AFTERDUSK_JPEG").map(PathBuf::from) else {
        eprintln!("AFTERDUSK_JPEG absent : rien à mesurer");
        return;
    };
    let started = Instant::now();
    let developed = rendered::develop(&path, 1).unwrap();
    let decoded_ms = started.elapsed().as_millis();
    let image = WorkingImage::from_raw(&developed, PREVIEW_SIDE);
    println!(
        "{} × {} : décodé en XYZ en {decoded_ms} ms, image de travail {} × {} en {} ms au total",
        developed.width,
        developed.height,
        image.width,
        image.height,
        started.elapsed().as_millis()
    );
}
