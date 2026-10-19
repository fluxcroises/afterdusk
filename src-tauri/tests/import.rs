//! Import de bout en bout : rangement, renommage, sauvegarde, vérification,
//! doublons, reprise après interruption et règles absolues.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use afterdusk_lib::import::{
    self, ImportError, ImportOptions, ImportOutcome, copy::PART_SUFFIX, scan,
};
use afterdusk_lib::library;
use afterdusk_lib::metadata::xmp;
use afterdusk_lib::{db, import::copy::hash_file};
use rusqlite::Connection;

struct Setup {
    _dir: tempfile::TempDir,
    card: PathBuf,
    root: PathBuf,
    backup: PathBuf,
    conn: Connection,
    library_id: i64,
}

/// Carte factice : deux vrais RAW, un JPEG sans EXIF, un RAW illisible, une vidéo.
fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte");
    let dcim = card.join("DCIM");
    fs::create_dir_all(dcim.join("100CANON")).unwrap();
    fs::create_dir_all(dcim.join("101NIKON")).unwrap();
    fs::copy(
        common::sample("canon_eos_r6.CR3"),
        dcim.join("100CANON").join("IMG_0001.CR3"),
    )
    .unwrap();
    fs::copy(
        common::sample("nikon_z6.NEF"),
        dcim.join("101NIKON").join("DSC_0750.NEF"),
    )
    .unwrap();
    fs::write(
        dcim.join("100CANON").join("IMG_0002.JPG"),
        jpeg_without_exif(),
    )
    .unwrap();
    fs::write(
        dcim.join("100CANON").join("IMG_0003.CR3"),
        vec![7u8; 200_000],
    )
    .unwrap();
    fs::write(dcim.join("100CANON").join("MVI_0004.MP4"), b"video").unwrap();

    let root = dir.path().join("Bibliotheque");
    let backup = dir.path().join("Sauvegarde");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&backup).unwrap();
    let conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, Some(&backup)).unwrap();
    Setup {
        _dir: dir,
        card,
        root,
        backup,
        conn,
        library_id: lib.id,
    }
}

fn jpeg_without_exif() -> Vec<u8> {
    // SOI, APP0 minimal, EOI : suffisant pour un fichier « image » sans EXIF.
    vec![
        0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0,
        0xff, 0xd9,
    ]
}

fn options(s: &Setup, files: Vec<PathBuf>) -> ImportOptions {
    ImportOptions {
        library_id: s.library_id,
        source_path: s.card.clone(),
        source_label: "Carte SD".into(),
        files,
        album_name: "Sortie test".into(),
        album_per_day: true,
        rename: true,
        backup: true,
        keywords: vec!["test".into()],
        creator: Some("Lewis Bedar".into()),
        rights: Some("© 2026 Lewis Bedar".into()),
    }
}

/// État complet de la source : contenu, date de modification, liste des fichiers.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, (String, SystemTime)> {
    walkdir(dir)
        .into_iter()
        .map(|p| {
            let modified = fs::metadata(&p).unwrap().modified().unwrap();
            (p.clone(), (hash_file(&p).unwrap(), modified))
        })
        .collect()
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walkdir(&path));
        } else {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn scanned(s: &Setup) -> Vec<PathBuf> {
    scan::scan(&s.card)
        .unwrap()
        .files
        .into_iter()
        .map(|f| f.path)
        .collect()
}

#[test]
fn import_complet_avec_sauvegarde_et_xmp() {
    let mut s = setup();
    let before = snapshot(&s.card);

    let result = scan::scan(&s.card).unwrap();
    assert_eq!(result.files.len(), 4);
    assert_eq!(result.ignored, 1);

    let opts = options(&s, scanned(&s));
    let id = import::plan(&mut s.conn, &opts).unwrap();
    let mut events = Vec::new();
    let outcome = import::run(&mut s.conn, id, &AtomicBool::new(false), |p| {
        events.push(p.clone())
    })
    .unwrap();
    assert_eq!(outcome, ImportOutcome::Done);
    let last = events.last().unwrap();
    assert_eq!((last.imported, last.failed, last.processed), (4, 0, 4));
    assert!(events.iter().filter(|e| e.media_id.is_some()).count() == 4);

    // Rangement par jour et renommage selon le modèle du cahier des charges ;
    // la numérotation suit l'ordre de prise de vue (le Nikon de 2019 d'abord).
    let nikon = s
        .root
        .join("2019")
        .join("2019-11-29 Sortie test")
        .join("20191129_171355_Z-6_0001.NEF");
    assert!(nikon.exists(), "{:?}", walkdir(&s.root));
    let canon = s
        .root
        .join("2021")
        .join("2021-05-12 Sortie test")
        .join("20210512_121633_EOS-R6_0002.CR3");
    assert!(canon.exists(), "{:?}", walkdir(&s.root));

    // Copie identique, sauvegarde identique.
    let source = s.card.join("DCIM").join("100CANON").join("IMG_0001.CR3");
    let rel = canon.strip_prefix(&s.root).unwrap();
    assert_eq!(hash_file(&canon).unwrap(), hash_file(&source).unwrap());
    assert_eq!(
        hash_file(&s.backup.join(rel)).unwrap(),
        hash_file(&source).unwrap()
    );

    // XMP : IMG.xmp pour un RAW, nom d'origine et copyright.
    let sidecar = canon.with_extension("xmp");
    let data = xmp::read_sidecar(&sidecar).unwrap().unwrap();
    assert_eq!(data.original_file_name.as_deref(), Some("IMG_0001.CR3"));
    assert_eq!(data.rights.as_deref(), Some("© 2026 Lewis Bedar"));
    assert_eq!(data.keywords, ["test"]);
    // IMG.JPG.xmp pour un JPEG.
    let jpeg_sidecars: Vec<_> = walkdir(&s.root)
        .into_iter()
        .filter(|p| p.to_string_lossy().ends_with(".JPG.xmp"))
        .collect();
    assert_eq!(jpeg_sidecars.len(), 1);

    // Base : médias, EXIF, album, boîtiers, aucun fichier temporaire.
    assert_eq!(count(&s.conn, "SELECT count(*) FROM media"), 4);
    assert_eq!(
        count(
            &s.conn,
            "SELECT count(*) FROM media_exif WHERE iso IS NOT NULL"
        ),
        2
    );
    assert_eq!(
        count(&s.conn, "SELECT count(*) FROM gear WHERE kind = 'body'"),
        2
    );
    assert_eq!(
        count(
            &s.conn,
            "SELECT count(*) FROM media WHERE path LIKE '2021/2021-05-12 Sortie test/%'"
        ),
        1
    );
    assert!(
        !walkdir(&s.root)
            .iter()
            .any(|p| p.to_string_lossy().ends_with(PART_SUFFIX))
    );

    // Règle absolue : aucune écriture sur la source.
    assert_eq!(snapshot(&s.card), before);
}

#[test]
fn second_import_reconnait_les_photos_deja_importees() {
    let mut s = setup();
    let files = scanned(&s);
    let opts = options(&s, files.clone());
    let id = import::plan(&mut s.conn, &opts).unwrap();
    import::run(&mut s.conn, id, &AtomicBool::new(false), |_| {}).unwrap();

    let known = import::known_fingerprints(&s.conn, s.library_id).unwrap();
    for file in scan::scan(&s.card).unwrap().files {
        assert!(known.contains(&file.fingerprint), "{}", file.file_name);
    }

    let opts = options(&s, files);

    let id = import::plan(&mut s.conn, &opts).unwrap();
    let mut last = None;
    import::run(&mut s.conn, id, &AtomicBool::new(false), |p| {
        last = Some(p.clone())
    })
    .unwrap();
    let last = last.unwrap();
    assert_eq!((last.duplicates, last.imported), (4, 0));
    assert_eq!(count(&s.conn, "SELECT count(*) FROM media"), 4);
}

#[test]
fn reprise_apres_interruption() {
    let mut s = setup();
    let before = snapshot(&s.card);
    let opts = options(&s, scanned(&s));
    let id = import::plan(&mut s.conn, &opts).unwrap();

    // Interruption après deux fichiers.
    let cancel = AtomicBool::new(false);
    let outcome = import::run(&mut s.conn, id, &cancel, |p| {
        if p.processed == 2 {
            cancel.store(true, Ordering::SeqCst);
        }
    })
    .unwrap();
    assert_eq!(outcome, ImportOutcome::Interrupted);
    assert_eq!(count(&s.conn, "SELECT count(*) FROM media"), 2);

    // Simule une coupure en pleine copie : un fichier temporaire tronqué.
    let pending: String = s
        .conn
        .query_row(
            "SELECT target_path FROM import_items WHERE status = 'pending' ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let target = library::paths::to_absolute(&s.root, &pending).unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(import::copy::part_path(&target), b"tronque").unwrap();

    // Au redémarrage, l'import est proposé à la reprise.
    import::mark_interrupted(&s.conn).unwrap();
    let resumable = import::resumable_imports(&s.conn).unwrap();
    assert_eq!(resumable.len(), 1);
    assert_eq!(resumable[0].remaining, 2);

    let outcome = import::run(&mut s.conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(outcome, ImportOutcome::Done);
    assert_eq!(count(&s.conn, "SELECT count(*) FROM media"), 4);
    assert!(import::resumable_imports(&s.conn).unwrap().is_empty());
    assert!(
        !walkdir(&s.root)
            .iter()
            .any(|p| p.to_string_lossy().ends_with(PART_SUFFIX))
    );
    // Chaque copie correspond à sa source.
    let mut stmt = s
        .conn
        .prepare("SELECT path, content_hash FROM media")
        .unwrap();
    for row in stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
    {
        let (path, hash) = row.unwrap();
        let abs = library::paths::to_absolute(&s.root, &path).unwrap();
        assert_eq!(hash_file(&abs).unwrap(), hash);
    }
    assert_eq!(snapshot(&s.card), before);
}

#[test]
fn carte_retiree_pendant_l_import() {
    let mut s = setup();
    let opts = options(&s, scanned(&s));
    let id = import::plan(&mut s.conn, &opts).unwrap();

    // La carte « disparaît » après le premier fichier.
    let hidden = s.card.with_file_name("carte-retiree");
    let cancel = AtomicBool::new(false);
    let mut first = true;
    let result = import::run(&mut s.conn, id, &cancel, |p| {
        if p.processed == 1 && first {
            first = false;
            fs::rename(&s.card, &hidden).unwrap();
        }
    });
    assert!(matches!(result, Err(ImportError::SourceUnavailable(_))));
    assert_eq!(import::resumable_imports(&s.conn).unwrap().len(), 1);

    // La carte revient : l'import se termine.
    fs::rename(&hidden, &s.card).unwrap();
    let outcome = import::run(&mut s.conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(outcome, ImportOutcome::Done);
    assert_eq!(count(&s.conn, "SELECT count(*) FROM media"), 4);
}

#[test]
fn coupure_entre_la_copie_et_la_base() {
    let mut s = setup();
    let files = vec![s.card.join("DCIM").join("100CANON").join("IMG_0001.CR3")];
    let opts = options(&s, files);
    let id = import::plan(&mut s.conn, &opts).unwrap();
    import::run(&mut s.conn, id, &AtomicBool::new(false), |_| {}).unwrap();

    // Le fichier est copié mais la base n'en garde pas trace.
    s.conn.execute("DELETE FROM media_exif", []).unwrap();
    s.conn.execute("DELETE FROM media", []).unwrap();
    s.conn
        .execute(
            "UPDATE import_items SET status = 'pending', media_id = NULL",
            [],
        )
        .unwrap();
    s.conn
        .execute("UPDATE imports SET status = 'interrupted'", [])
        .unwrap();

    import::run(&mut s.conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(count(&s.conn, "SELECT count(*) FROM media"), 1);
    // Le fichier déjà présent est reconnu : pas de copie « -2 ».
    let raws: Vec<_> = walkdir(&s.root)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "CR3"))
        .collect();
    assert_eq!(raws.len(), 1, "{raws:?}");
}

#[test]
fn bibliotheque_hors_ligne() {
    let mut s = setup();
    let files = scanned(&s);
    let moved = s.root.with_file_name("Bibliotheque-debranchee");
    fs::rename(&s.root, &moved).unwrap();
    let opts = options(&s, files);
    assert!(matches!(
        import::plan(&mut s.conn, &opts),
        Err(ImportError::LibraryOffline(_))
    ));
}

/// 500 fichiers, coupure au milieu, reprise : aucun fichier perdu ni en double.
#[test]
fn cinq_cents_fichiers_sans_perte() {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("carte").join("DCIM").join("100_FUJI");
    fs::create_dir_all(&card).unwrap();
    for i in 0..500u32 {
        // Contenus distincts pour des empreintes distinctes.
        let mut data = vec![0u8; 20_000];
        data[..4].copy_from_slice(&i.to_le_bytes());
        fs::write(card.join(format!("DSCF{i:04}.RAF")), data).unwrap();
    }
    let root = dir.path().join("Bibliotheque");
    fs::create_dir_all(&root).unwrap();
    let mut conn = db::open(&dir.path().join("afterdusk.db")).unwrap();
    let lib = library::create_library(&conn, "Photos", &root, None).unwrap();
    let files: Vec<PathBuf> = scan::scan(&card)
        .unwrap()
        .files
        .into_iter()
        .map(|f| f.path)
        .collect();
    assert_eq!(files.len(), 500);
    let opts = ImportOptions {
        library_id: lib.id,
        source_path: card.clone(),
        source_label: "Carte".into(),
        files,
        album_name: "Volume".into(),
        album_per_day: false,
        rename: false,
        backup: false,
        keywords: vec![],
        creator: None,
        rights: None,
    };
    let id = import::plan(&mut conn, &opts).unwrap();

    let cancel = AtomicBool::new(false);
    let outcome = import::run(&mut conn, id, &cancel, |p| {
        if p.processed == 250 {
            cancel.store(true, Ordering::SeqCst);
        }
    })
    .unwrap();
    assert_eq!(outcome, ImportOutcome::Interrupted);
    import::run(&mut conn, id, &AtomicBool::new(false), |_| {}).unwrap();

    assert_eq!(count(&conn, "SELECT count(*) FROM media"), 500);
    assert_eq!(
        count(&conn, "SELECT count(DISTINCT content_hash) FROM media"),
        500
    );
    let copies = walkdir(&root)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "RAF"))
        .count();
    assert_eq!(copies, 500);
}

/// Après l'import : aperçus en cache, albums, fiche d'informations.
#[test]
fn apercus_et_fiche_apres_import() {
    use afterdusk_lib::library::media;

    let mut s = setup();
    let opts = options(&s, scanned(&s));
    let id = import::plan(&mut s.conn, &opts).unwrap();
    let mut imported = Vec::new();
    import::run(&mut s.conn, id, &AtomicBool::new(false), |p| {
        imported.extend(p.media_id)
    })
    .unwrap();

    let cache = s.root.with_file_name("cache");
    assert_eq!(media::missing_previews(&s.conn).unwrap().len(), 4);
    let mut generated = 0;
    for media_id in &imported {
        // Le RAW illisible et le JPEG vide n'ont pas d'aperçu : c'est toléré.
        if media::generate_previews(&s.conn, &cache, *media_id).is_ok() {
            generated += 1;
        }
    }
    assert_eq!(generated, 2);
    assert_eq!(media::missing_previews(&s.conn).unwrap().len(), 2);

    let list = media::list_media(&s.conn, s.library_id, None).unwrap();
    assert_eq!(list.len(), 4);
    assert_eq!(list.iter().filter(|m| m.has_preview).count(), 2);
    for m in list.iter().filter(|m| m.has_preview) {
        assert!(media::cached_preview(&cache, m.id, 256).is_some());
    }

    // Illustration de l'accueil : seulement des photos avec un aperçu 2048 px.
    let highlights = media::highlights(&s.conn, s.library_id, 12).unwrap();
    assert_eq!(highlights.len(), 2);

    let recent = import::recent_imports(&s.conn, s.library_id, 5, 3).unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!((recent[0].count, recent[0].media_ids.len()), (4, 3));

    // Zoom 100 % : JPEG pleine taille intégré au CR3, original intact.
    let canon_path: String = s
        .conn
        .query_row(
            "SELECT path FROM media WHERE path LIKE '%EOS-R6%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let canon_abs = library::paths::to_absolute(&s.root, &canon_path).unwrap();
    let before = common::sha256(&canon_abs);
    let canon_id: i64 = s
        .conn
        .query_row("SELECT id FROM media WHERE path = ?1", [&canon_path], |r| {
            r.get(0)
        })
        .unwrap();
    let full = media::full_size_jpeg(&s.conn, canon_id).unwrap().unwrap();
    let img = image::load_from_memory(&full).unwrap();
    assert_eq!((img.width(), img.height()), (5472, 3648));
    assert_eq!(common::sha256(&canon_abs), before);

    let albums = media::list_albums(&s.conn, s.library_id).unwrap();
    assert_eq!(albums.iter().map(|a| a.count).sum::<i64>(), 4);
    assert!(
        albums
            .iter()
            .all(|a| a.cover_media_id.is_some() && !a.cover_chosen)
    );

    // Couverture choisie : seulement une photo de l'album.
    let (album_a, album_b) = (albums[0].id, albums[1].id);
    let in_b: i64 = s
        .conn
        .query_row(
            "SELECT id FROM media WHERE album_id = ?1 LIMIT 1",
            [album_b],
            |r| r.get(0),
        )
        .unwrap();
    assert!(library::set_album_cover(&s.conn, album_a, in_b).is_err());
    library::set_album_cover(&s.conn, album_b, in_b).unwrap();
    let albums = media::list_albums(&s.conn, s.library_id).unwrap();
    assert_eq!(
        albums
            .iter()
            .find(|a| a.id == album_b)
            .unwrap()
            .cover_media_id,
        Some(in_b)
    );

    let canon = list
        .iter()
        .find(|m| m.file_name.contains("EOS-R6"))
        .unwrap();
    let details = media::media_details(&s.conn, canon.id).unwrap().unwrap();
    assert_eq!(details.camera.as_deref(), Some("Canon EOS R6"));
    assert_eq!(details.original_name, "IMG_0001.CR3");
    assert_eq!(details.keywords, ["test"]);
    assert_eq!(details.iso, Some(100.0));
}

/// Notes, drapeaux et couleurs : en base puis dans le XMP, original intact.
#[test]
fn marques_ecrites_dans_le_xmp_sans_toucher_l_original() {
    use afterdusk_lib::library::marks::{self, Mark};
    use afterdusk_lib::metadata::xmp::{ColorLabel, Flag};

    let mut s = setup();
    let files = vec![s.card.join("DCIM").join("100CANON").join("IMG_0001.CR3")];
    let opts = options(&s, files);
    let id = import::plan(&mut s.conn, &opts).unwrap();
    let mut media_id = 0;
    import::run(&mut s.conn, id, &AtomicBool::new(false), |p| {
        if let Some(m) = p.media_id {
            media_id = m;
        }
    })
    .unwrap();

    let (path, sidecar): (String, String) = s
        .conn
        .query_row(
            "SELECT path, sidecar_path FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let original = library::paths::to_absolute(&s.root, &path).unwrap();
    let before = common::sha256(&original);
    let modified = common::modified(&original);

    marks::apply_mark(&mut s.conn, &[media_id], Mark::Rating(5)).unwrap();
    marks::apply_mark(&mut s.conn, &[media_id], Mark::Flag(Flag::Rejected)).unwrap();
    marks::apply_mark(&mut s.conn, &[media_id], Mark::Color(Some(ColorLabel::Red))).unwrap();
    assert_eq!(marks::flush_pending_xmp(&s.conn, 10).unwrap().written, 1);

    let data = xmp::read_sidecar(&library::paths::to_absolute(&s.root, &sidecar).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        (data.rating, data.flag, data.label),
        (5, Flag::Rejected, Some(ColorLabel::Red))
    );
    // Les données écrites à l'import sont conservées.
    assert_eq!(data.original_file_name.as_deref(), Some("IMG_0001.CR3"));
    assert_eq!(data.keywords, ["test"]);
    // Règle absolue : l'original n'est pas modifié.
    assert_eq!(common::sha256(&original), before);
    assert_eq!(common::modified(&original), modified);
}

/// Un XMP laissé par un autre logiciel : sa note est reprise en base à l'import.
#[test]
fn note_d_un_xmp_existant_reprise_a_l_import() {
    use afterdusk_lib::metadata::xmp::{ColorLabel, Flag, SidecarData};

    let mut s = setup();
    let files = vec![s.card.join("DCIM").join("100CANON").join("IMG_0001.CR3")];
    // Le fichier de destination prévu : on y dépose d'avance un XMP « Lightroom ».
    let opts = options(&s, files);
    let id = import::plan(&mut s.conn, &opts).unwrap();
    let target: String = s
        .conn
        .query_row(
            "SELECT target_path FROM import_items WHERE import_id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    let abs = library::paths::to_absolute(&s.root, &target).unwrap();
    fs::create_dir_all(abs.parent().unwrap()).unwrap();
    xmp::write_sidecar(
        &abs.with_extension("xmp"),
        &SidecarData {
            rating: 3,
            flag: Flag::Picked,
            label: Some(ColorLabel::Blue),
            keywords: vec!["Bretagne".into()],
            ..SidecarData::default()
        },
    )
    .unwrap();

    import::run(&mut s.conn, id, &AtomicBool::new(false), |_| {}).unwrap();
    let (rating, flag, color): (i64, i64, Option<String>) = s
        .conn
        .query_row("SELECT rating, flag, color FROM media", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!((rating, flag, color.as_deref()), (3, 1, Some("blue")));
    assert_eq!(
        count(
            &s.conn,
            "SELECT count(*) FROM keywords WHERE word IN ('Bretagne', 'test')"
        ),
        2
    );
}
