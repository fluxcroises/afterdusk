//! Affiche les métadonnées d'un RAW et extrait son aperçu intégré.
//!
//!   cargo run --example raw_probe -- <fichier> [dossier de sortie]

use std::path::PathBuf;
use std::time::Instant;

use afterdusk_lib::raw::{PreviewFormat, RawFile, libraw_version};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(args.next().ok_or("usage : raw_probe <fichier> [sortie]")?);
    let out_dir = args.next().map(PathBuf::from);

    let start = Instant::now();
    let mut raw = RawFile::open(&path)?;
    let info = raw.info();
    let opened = start.elapsed();
    let preview = raw.preview()?;
    let total = start.elapsed();

    println!("LibRaw {}", libraw_version());
    println!("{}", serde_json::to_string_pretty(&info)?);
    println!("balises EXIF relevées : {}", raw.exif().len());
    for p in raw.embedded_previews() {
        println!(
            "  aperçu {} : {} {}x{}, {} Ko",
            p.index,
            if p.is_jpeg { "JPEG" } else { "autre" },
            p.width,
            p.height,
            p.length / 1024
        );
    }
    println!(
        "aperçu : {:?} {}x{}, {} Ko",
        preview.format,
        preview.width,
        preview.height,
        preview.data.len() / 1024
    );
    println!(
        "temps : ouverture {} ms, aperçu {} ms",
        opened.as_millis(),
        (total - opened).as_millis()
    );

    if let Some(dir) = out_dir {
        let ext = match preview.format {
            PreviewFormat::Jpeg => "jpg",
            PreviewFormat::JpegXl => "jxl",
            PreviewFormat::Bitmap => "rgb",
        };
        let stem = path.file_name().ok_or("nom de fichier")?.to_string_lossy();
        let target = dir.join(format!("{stem}.{ext}"));
        std::fs::write(&target, &preview.data)?;
        println!("écrit : {}", target.display());
    }
    Ok(())
}
