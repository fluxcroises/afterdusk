//! Liste les balises EXIF relevées dans un RAW.
//!
//!   cargo run --example exif_dump -- <fichier>

use std::path::PathBuf;

use afterdusk_lib::raw::RawFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage : exif_dump <fichier>")?,
    );
    let raw = RawFile::open(&path)?;
    for ((ifd, tag), value) in raw.exif() {
        println!("{ifd:?} 0x{tag:04x} {value}");
    }
    Ok(())
}
