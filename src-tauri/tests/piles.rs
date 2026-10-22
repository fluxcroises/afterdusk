//! Paires RAW + JPEG en pile (décision 0003) : import, marques, corbeille,
//! dessus de pile, aperçus et migration.

mod common;

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use afterdusk_lib::db;
use afterdusk_lib::import::copy::hash_file;
use afterdusk_lib::import::{self, ImportOptions};
use afterdusk_lib::library::marks::{Mark, apply_mark, flush_pending_xmp};
use afterdusk_lib::library::media::{self, MediaQuery, list_media};
use afterdusk_lib::library::{self, pairs, paths, trash};
use afterdusk_lib::metadata::media::MediaFormat;
use afterdusk_lib::metadata::xmp;
use exif::experimental::Writer;
use exif::{Field, In, Tag, Value};
use rusqlite::Connection;

/// JPEG « du boîtier » : image unie avec les EXIF utiles (date, dimensions).
fn camera_jpeg(path: &Path, width: u32, height: u32, taken: &str) {
    let mut jpeg = Vec::new();
    let img = image::RgbImage::from_pixel(width, height, image::Rgb([90, 120, 160]));
    img.write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .unwrap();
    let fields = [
        Field {
            tag: Tag::DateTimeOriginal,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![taken.as_bytes().to_vec()]),
        },
        Field {
            tag: Tag::PixelXDimension,
            ifd_num: In::PRIMARY,
            value: Value::Long(vec![width]),
        },
        Field {
            tag: Tag::PixelYDimension,
            ifd_num: In::PRIMARY,
            value: Value::Long(vec![height]),
        },
    ];
    let mut writer = Writer::new();
    for f in &fields {
        writer.push_field(f);
    }
    let mut tiff = Cursor::new(Vec::new());
    writer.write(&mut tiff, false).unwrap();
    let tiff = tiff.into_inner();
    // APP1 « Exif » juste après le marqueur de début d'image.
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(&tiff);
    out.extend_from_slice(&jpeg[2..]);
    fs::write(path, out).unwrap();
}

struct Setup {
    dir: tempfile::TempDir,
    root: PathBuf,
    cache: PathBuf,
    conn: Connection,
    library_id: i64,
}

impl Setup {
    fn heads(&self) -> Vec<(String, i64)> {
        list_media(&self.conn, self.library_id, &MediaQuery::default())
            .unwrap()
            .into_iter()
            .map(|m| (m.file_name, m.stack_size))
            .collect()
    }

    fn id(&self, suffix: &str) -> i64 {
        self.conn
            .query_row(
                "SELECT id FROM media WHERE lower(path) LIKE '%' || lower(?1)",
                [suffix],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn path(&self, id: i64) -> PathBuf {
        let rel: String = self
            .conn
            .query_row("SELECT path FROM media WHERE id = ?1", [id], |r| r.get(0))
            .unwrap();
        paths::to_absolute(&self.root, &rel).unwrap()
    }
}

/// Carte : Sony ARW + son JPEG (même heure), Nikon NEF + un JPEG homonyme pris
/// plus tard (compteur repassé), et un JPEG seul.
fn setup(rename: bool, skip_pair_jpegs: bool) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte").join("DCIM");
    fs::create_dir_all(&card).unwrap();
    fs::copy(
        common::sample("sony_ilce-7m3.ARW"),
        card.join("DSC09001.ARW"),
    )
    .unwrap();
    camera_jpeg(
        &card.join("dsc09001.jpg"),
        3000,
        2000,
        "2018:03:13 16:38:13",
    );
    fs::copy(common::sample("nikon_z6.NEF"), card.join("DSC_0750.NEF")).unwrap();
    camera_jpeg(&card.join("DSC_0750.JPG"), 600, 400, "2020:01:01 10:00:00");
    camera_jpeg(&card.join("IMG_0001.JPG"), 600, 400, "2021:05:12 09:00:00");

    let root = dir.path().join("Bibliotheque");
    let cache = dir.path().join("cache");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let mut files: Vec<PathBuf> = fs::read_dir(&card)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    let opts = ImportOptions {
        library_id: lib.id,
        source_path: card,
        source_label: "Carte".into(),
        files,
        album_name: "Sortie".into(),
        album_per_day: false,
        rename,
        backup: false,
        keywords: vec![],
        creator: None,
        rights: None,
        skip_pair_jpegs,
        client_id: None,
        session_type_id: None,
        session_label: String::new(),
    };
    let id = import::plan(&mut conn, &opts).unwrap();
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    Setup {
        dir,
        root,
        cache,
        conn,
        library_id: lib.id,
    }
}

#[test]
fn import_forme_les_piles() {
    let s = setup(false, false);
    let mut heads = s.heads();
    heads.sort();
    assert_eq!(
        heads,
        [
            ("DSC09001.ARW".into(), 2),
            ("DSC_0750.JPG".into(), 1),
            ("DSC_0750.NEF".into(), 1),
            ("IMG_0001.JPG".into(), 1),
        ]
    );
    // La pile compte pour une photo dans l'album.
    let albums = media::list_albums(&s.conn, s.library_id).unwrap();
    assert_eq!(albums[0].count, 4);
    // Pile dépliée : le JPEG suit son RAW.
    let arw = s.id("DSC09001.ARW");
    let open = list_media(
        &s.conn,
        s.library_id,
        &MediaQuery {
            expanded: vec![arw],
            ..Default::default()
        },
    )
    .unwrap();
    let at = open.iter().position(|m| m.id == arw).unwrap();
    assert_eq!(open[at + 1].file_name, "dsc09001.jpg");
    assert_eq!(open[at + 1].companion_of, Some(arw));
    // La fiche nomme les deux fichiers.
    let details = media::media_details(&s.conn, arw).unwrap().unwrap();
    assert_eq!(details.stack_files, ["DSC09001.ARW", "dsc09001.jpg"]);
}

#[test]
fn renommage_un_numero_par_paire() {
    let s = setup(true, false);
    let arw = s.path(s.id(".ARW"));
    let jpg = s.path(pairs::members(&s.conn, s.id(".ARW"), None).unwrap()[1]);
    assert_eq!(arw.file_stem(), jpg.file_stem(), "{arw:?} / {jpg:?}");
    assert_eq!(jpg.extension().unwrap(), "jpg");
    // Quatre déclenchements (la paire en est un) : numéros 1 à 4, sans trou.
    let mut seqs: Vec<String> = fs::read_dir(arw.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| !n.ends_with(".xmp"))
        .map(|n| n.rsplit('_').next().unwrap()[..4].to_owned())
        .collect();
    seqs.sort();
    seqs.dedup();
    assert_eq!(seqs, ["0001", "0002", "0003", "0004"]);
}

#[test]
fn option_sans_les_jpeg_des_paires() {
    let s = setup(false, true);
    let mut names: Vec<String> = s.heads().into_iter().map(|(n, _)| n).collect();
    names.sort();
    // Le JPEG de la paire n'est pas copié ; les JPEG sans RAW le sont.
    assert_eq!(
        names,
        [
            "DSC09001.ARW",
            "DSC_0750.JPG",
            "DSC_0750.NEF",
            "IMG_0001.JPG"
        ]
    );
    let count: i64 = s
        .conn
        .query_row("SELECT count(*) FROM media", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 4);
}

#[test]
fn marques_sur_toute_la_pile_et_dans_les_deux_xmp() {
    let mut s = setup(false, false);
    let arw = s.id("DSC09001.ARW");
    apply_mark(&mut s.conn, &[arw], Mark::Rating(4)).unwrap();
    flush_pending_xmp(&s.conn, 100).unwrap();
    for id in pairs::members(&s.conn, arw, None).unwrap() {
        let (rating, sidecar): (i64, String) = s
            .conn
            .query_row(
                "SELECT rating, sidecar_path FROM media WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(rating, 4);
        let data = xmp::read_sidecar(&paths::to_absolute(&s.root, &sidecar).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(data.rating, 4, "{sidecar}");
    }
}

/// Règle absolue : chaque fichier de la pile passe par la corbeille et revient
/// à l'identique ; le fichier resté prend la place du dessus.
#[test]
fn corbeille_au_choix_du_format() {
    let mut s = setup(false, false);
    let arw = s.id("DSC09001.ARW");
    let jpg = s.id("dsc09001.jpg");
    let (arw_hash, jpg_hash) = (
        hash_file(&s.path(arw)).unwrap(),
        hash_file(&s.path(jpg)).unwrap(),
    );

    // « Envoyer le JPEG dans la corbeille » : le RAW reste, seul.
    let chosen = pairs::files_of_format(&s.conn, &[arw], MediaFormat::Image).unwrap();
    assert_eq!(chosen, [jpg]);
    trash::move_to_trash(&mut s.conn, &chosen).unwrap();
    assert!(s.heads().contains(&("DSC09001.ARW".into(), 1)));
    assert_eq!(trash::trashed_count(&s.conn, s.library_id).unwrap(), 1);
    trash::restore(&mut s.conn, &[jpg]).unwrap();
    assert!(s.heads().contains(&("DSC09001.ARW".into(), 2)));

    // « Envoyer le RAW dans la corbeille » : le JPEG passe dessus.
    let chosen = pairs::files_of_format(&s.conn, &[jpg], MediaFormat::Raw).unwrap();
    assert_eq!(chosen, [arw]);
    // Le rapport nomme le fichier déplacé : c'est lui qu'Annuler restaure.
    assert_eq!(
        trash::move_to_trash(&mut s.conn, &chosen).unwrap().moved,
        [arw]
    );
    assert!(s.heads().contains(&("dsc09001.jpg".into(), 1)));
    // Restauré, le RAW revient sous le JPEG ; « Mettre en haut » le remonte.
    trash::restore(&mut s.conn, &[arw]).unwrap();
    assert!(s.heads().contains(&("dsc09001.jpg".into(), 2)));
    pairs::set_top(&mut s.conn, arw).unwrap();
    assert!(s.heads().contains(&("DSC09001.ARW".into(), 2)));

    // Une photo seule n'est jamais vidée par ce choix.
    let nef = s.id("DSC_0750.NEF");
    assert!(
        pairs::files_of_format(&s.conn, &[nef], MediaFormat::Raw)
            .unwrap()
            .is_empty()
    );
    assert_eq!(hash_file(&s.path(arw)).unwrap(), arw_hash);
    assert_eq!(hash_file(&s.path(jpg)).unwrap(), jpg_hash);
}

#[test]
fn pile_entiere_en_corbeille_puis_videe() {
    let mut s = setup(false, false);
    let arw = s.id("DSC09001.ARW");
    let both = pairs::expand(&s.conn, &[arw], Some("normal")).unwrap();
    assert_eq!(both.len(), 2);
    trash::move_to_trash(&mut s.conn, &both).unwrap();
    assert_eq!(s.heads().len(), 3);
    // Dans la corbeille, la pile reste une pile.
    let in_trash = list_media(
        &s.conn,
        s.library_id,
        &MediaQuery {
            trashed: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(in_trash.len(), 1);
    assert_eq!(in_trash[0].stack_size, 2);
    let purge = pairs::expand(&s.conn, &[in_trash[0].id], Some("trashed")).unwrap();
    trash::purge(&mut s.conn, &s.cache, &purge).unwrap();
    let left: i64 = s
        .conn
        .query_row("SELECT count(*) FROM media", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 3);
}

#[test]
fn apercus_tires_du_jpeg_du_boitier() {
    let s = setup(false, false);
    let arw = s.id("DSC09001.ARW");
    // L'aperçu intégré de l'ARW ne fait que 1616 px ; le JPEG en fait 3000.
    let generated = media::generate_previews(&s.conn, &s.cache, arw).unwrap();
    let largest = generated.iter().map(|g| g.width.max(g.height)).max();
    assert_eq!(largest, Some(2048));
    // Zoom 100 % : le JPEG du boîtier, lu tel quel.
    let full = media::full_size_jpeg(&s.conn, arw).unwrap().unwrap();
    assert_eq!(full, fs::read(s.path(s.id("dsc09001.jpg"))).unwrap());
    // Une photo seule garde son aperçu intégré.
    let nef = s.id("DSC_0750.NEF");
    let generated = media::generate_previews(&s.conn, &s.cache, nef).unwrap();
    assert!(!generated.is_empty());
    drop(s.dir);
}

#[test]
fn migration_relie_les_paires_deja_importees() {
    let s = setup(false, false);
    // Base d'avant la version 0.3 du schéma : aucun lien.
    s.conn
        .execute("UPDATE media SET companion_of = NULL", [])
        .unwrap();
    let sql = include_str!("../src/db/migrations/0003_piles_raw_jpeg.sql");
    let relink = &sql[sql.find("CREATE TEMP TABLE").unwrap()..];
    s.conn.execute_batch(relink).unwrap();
    let mut heads = s.heads();
    heads.sort();
    assert_eq!(heads[0], ("DSC09001.ARW".into(), 2));
    assert_eq!(heads.len(), 4);
}
