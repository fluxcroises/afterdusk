//! Outil de développement : importe un dossier dans une base Afterdusk, avec le
//! même moteur que l'application, pour préparer un essai de bout en bout.
//!
//!   cargo run --example dev_seed -- <base.db> <racine de bibliothèque> <dossier source>

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use afterdusk_lib::import::{self, ImportOptions, scan};
use afterdusk_lib::{db, library};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let usage = "usage : dev_seed <base.db> <bibliothèque> <source>";
    let (db_path, root, source) = (
        args.next().ok_or(usage)?,
        args.next().ok_or(usage)?,
        args.next().ok_or(usage)?,
    );

    std::fs::create_dir_all(&root)?;
    let mut conn = db::open(&db_path)?;
    let lib = match library::list_libraries(&conn)?
        .into_iter()
        .find(|l| l.root_path == root)
    {
        Some(lib) => lib,
        None => library::create_library(&conn, "Essai", &root, None)?,
    };
    let files = scan::scan(&source)?
        .files
        .into_iter()
        .map(|f| f.path)
        .collect();
    let options = ImportOptions {
        library_id: lib.id,
        source_path: source.clone(),
        source_label: "Essai".into(),
        files,
        album_name: "Essai".into(),
        album_per_day: true,
        rename: false,
        backup: false,
        keywords: vec![],
        creator: None,
        rights: None,
    };
    let id = import::plan(&mut conn, &options)?;
    let outcome = import::run(&mut conn, id, &AtomicBool::new(false), |p| {
        println!("{}/{} {:?}", p.processed, p.total, p.current_file);
    })?;
    println!("import {id} : {outcome:?}");
    Ok(())
}
