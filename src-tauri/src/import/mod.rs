//! Import : analyse de la source, planification, copie vérifiée, sauvegarde,
//! XMP et enregistrement en base, avec reprise après coupure.
//!
//! Chaque fichier avance dans `import_items` par étapes enregistrées
//! (`pending` → `verified` → `backed_up` → `done`) ; chaque étape peut être
//! rejouée sans dommage, ce qui permet de reprendre un import interrompu
//! là où il s'était arrêté. La source n'est jamais ouverte en écriture.

pub mod copy;
pub mod scan;

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::now_utc;
use crate::library::paths::{self, PathError};
use crate::library::template::{self, TemplateValues};
use crate::library::{self, GearKind, Library, LibraryError};
use crate::metadata::media::{MediaFormat, MediaMetadata, format_of};
use crate::metadata::xmp::{self, SidecarData, SidecarNaming};
use copy::CopyError;
use scan::{ScanError, SourceFile};

/// Modèle de renommage proposé par défaut (cahier des charges, section 4.1).
pub const DEFAULT_RENAME_TEMPLATE: &str = "{yyyy}{mm}{dd}_{hh}{min}{ss}_{camera}_{seq:4}";

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("erreur SQLite : {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Scan(#[from] ScanError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Template(#[from] template::TemplateError),
    #[error("la bibliothèque {0} est hors ligne : rebranchez le disque")]
    LibraryOffline(PathBuf),
    #[error("{0} n'est pas un média reconnu")]
    UnsupportedFile(PathBuf),
    #[error("la source n'est plus accessible ({0}) : l'import pourra être repris")]
    SourceUnavailable(PathBuf),
    #[error("import {0} introuvable")]
    NotFound(i64),
    #[error("options d'import illisibles : {0}")]
    Options(#[from] serde_json::Error),
}

/// Ce que l'utilisateur a choisi dans l'écran Importer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOptions {
    pub library_id: i64,
    pub source_path: PathBuf,
    pub source_label: String,
    /// Fichiers sélectionnés sur la source.
    pub files: Vec<PathBuf>,
    pub album_name: String,
    /// Un album (un dossier) par jour de prise de vue ; sinon un seul album
    /// daté du premier jour.
    pub album_per_day: bool,
    pub rename: bool,
    pub backup: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub creator: Option<String>,
    pub rights: Option<String>,
}

/// Options conservées en base pour la reprise (sans la liste des fichiers).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredOptions {
    album_name: String,
    backup: bool,
    keywords: Vec<String>,
    creator: Option<String>,
    rights: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub import_id: i64,
    pub total: usize,
    pub processed: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub failed: usize,
    pub current_file: Option<String>,
    /// Média tout juste enregistré : son aperçu peut être affiché.
    pub media_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportOutcome {
    Done,
    Interrupted,
}

/// Empreintes déjà présentes dans la bibliothèque (photos à griser).
pub fn known_fingerprints(
    conn: &Connection,
    library_id: i64,
) -> Result<HashSet<String>, ImportError> {
    let mut stmt = conn.prepare("SELECT fingerprint FROM media WHERE library_id = ?1")?;
    let set = stmt
        .query_map([library_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(set)
}

fn online_library(conn: &Connection, library_id: i64) -> Result<Library, ImportError> {
    let lib = library::get_library(conn, library_id)?;
    if !lib.is_online {
        return Err(ImportError::LibraryOffline(lib.root_path));
    }
    Ok(lib)
}

/// Prépare l'import : métadonnées, doublons, chemins de destination.
/// Rien n'est encore copié ; renvoie l'identifiant de l'import.
pub fn plan(conn: &mut Connection, options: &ImportOptions) -> Result<i64, ImportError> {
    let lib = online_library(conn, options.library_id)?;
    let known = known_fingerprints(conn, lib.id)?;

    let mut files: Vec<SourceFile> = options
        .files
        .iter()
        .map(|path| {
            let format =
                format_of(path).ok_or_else(|| ImportError::UnsupportedFile(path.clone()))?;
            Ok(scan::describe(path, format)?)
        })
        .collect::<Result<_, ImportError>>()?;
    files.sort_by(|a, b| (&a.captured_at, &a.file_name).cmp(&(&b.captured_at, &b.file_name)));

    let first_date = files
        .first()
        .map(|f| f.captured_at.clone())
        .unwrap_or_default();
    let rename_template = lib
        .rename_template
        .clone()
        .unwrap_or_else(|| DEFAULT_RENAME_TEMPLATE.to_owned());

    let mut taken: HashSet<String> = conn
        .prepare("SELECT lower(path) FROM media WHERE library_id = ?1")?
        .query_map([lib.id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;

    let stored = StoredOptions {
        album_name: options.album_name.trim().to_owned(),
        backup: options.backup && lib.backup_path.is_some(),
        keywords: options.keywords.clone(),
        creator: options.creator.clone(),
        rights: options.rights.clone(),
    };

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO imports (library_id, source_label, source_path, status, files_total,
                              backup_path, options, started_at)
         VALUES (?1, ?2, ?3, 'interrupted', ?4, ?5, ?6, ?7)",
        params![
            lib.id,
            options.source_label,
            options.source_path.to_string_lossy(),
            files.len() as i64,
            stored
                .backup
                .then(|| lib
                    .backup_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()))
                .flatten(),
            serde_json::to_string(&stored)?,
            now_utc()
        ],
    )?;
    let import_id = tx.last_insert_rowid();

    let mut sequence = 0u32;
    let mut seen_in_batch = HashSet::new();
    for file in &files {
        let duplicate =
            known.contains(&file.fingerprint) || !seen_in_batch.insert(file.fingerprint.clone());
        let target = if duplicate {
            String::new()
        } else {
            sequence += 1;
            let folder_date = if options.album_per_day {
                &file.captured_at
            } else {
                &first_date
            };
            let camera = file.metadata.camera_label();
            let stem = Path::new(&file.file_name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let values = TemplateValues {
                captured_at: folder_date,
                album: &stored.album_name,
                camera: &camera,
                original_stem: &stem,
                sequence,
            };
            let folder = template::render_folder(&lib.folder_template, &values)?;
            let name = if options.rename {
                let ext = Path::new(&file.file_name)
                    .extension()
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let values = TemplateValues {
                    captured_at: &file.captured_at,
                    ..values
                };
                template::render_file_name(&rename_template, &values, &ext)?
            } else {
                paths::sanitize_component(&file.file_name)
            };
            unique_target(&lib.root_path, &folder, &name, &mut taken)
        };
        tx.execute(
            "INSERT INTO import_items (import_id, source_path, source_size, source_mtime,
                                       fingerprint, target_path, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                import_id,
                file.path.to_string_lossy(),
                file.size as i64,
                file.modified,
                file.fingerprint,
                target,
                if duplicate { "duplicate" } else { "pending" },
                now_utc()
            ],
        )?;
    }
    tx.commit()?;
    Ok(import_id)
}

/// Chemin relatif libre : ni prévu dans cet import, ni en base, ni sur le disque.
fn unique_target(root: &Path, folder: &str, name: &str, taken: &mut HashSet<String>) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_owned(), format!(".{e}")),
        _ => (name.to_owned(), String::new()),
    };
    let mut n = 1;
    loop {
        let candidate = if n == 1 {
            format!("{folder}/{name}")
        } else {
            format!("{folder}/{stem}-{n}{ext}")
        };
        let free = !taken.contains(&candidate.to_lowercase())
            && paths::to_absolute(root, &candidate).is_ok_and(|p| !p.exists());
        if free {
            taken.insert(candidate.to_lowercase());
            return candidate;
        }
        n += 1;
    }
}

struct Item {
    id: i64,
    source: PathBuf,
    target: String,
    status: String,
    content_hash: Option<String>,
}

/// Passe les imports restés « en cours » (application fermée ou coupure) à « interrompu ».
pub fn mark_interrupted(conn: &Connection) -> Result<usize, ImportError> {
    Ok(conn.execute(
        "UPDATE imports SET status = 'interrupted' WHERE status = 'running'",
        [],
    )?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumableImport {
    pub id: i64,
    pub library_id: i64,
    pub source_label: String,
    pub source_path: PathBuf,
    pub started_at: String,
    pub remaining: i64,
    pub total: i64,
}

/// Imports interrompus qui ont encore des fichiers à traiter.
pub fn resumable_imports(conn: &Connection) -> Result<Vec<ResumableImport>, ImportError> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.library_id, i.source_label, i.source_path, i.started_at, i.files_total,
                (SELECT count(*) FROM import_items t
                  WHERE t.import_id = i.id AND t.status NOT IN ('done', 'duplicate', 'failed'))
         FROM imports i WHERE i.status = 'interrupted' ORDER BY i.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ResumableImport {
                id: r.get(0)?,
                library_id: r.get(1)?,
                source_label: r.get(2)?,
                source_path: PathBuf::from(r.get::<_, String>(3)?),
                started_at: r.get(4)?,
                total: r.get(5)?,
                remaining: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().filter(|r| r.remaining > 0).collect())
}

/// Exécute (ou reprend) un import. S'arrête proprement entre deux fichiers
/// si `cancel` passe à vrai, ou si la source disparaît (carte retirée).
pub fn run(
    conn: &mut Connection,
    import_id: i64,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(&ImportProgress),
) -> Result<ImportOutcome, ImportError> {
    let (library_id, options_json): (i64, String) = conn
        .query_row(
            "SELECT library_id, options FROM imports WHERE id = ?1",
            [import_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(ImportError::NotFound(import_id))?;
    let options: StoredOptions = serde_json::from_str(&options_json)?;
    let lib = online_library(conn, library_id)?;
    conn.execute(
        "UPDATE imports SET status = 'running' WHERE id = ?1",
        [import_id],
    )?;

    let mut progress = ImportProgress {
        import_id,
        ..ImportProgress::default()
    };
    for (status, count) in conn
        .prepare("SELECT status, count(*) FROM import_items WHERE import_id = ?1 GROUP BY status")?
        .query_map([import_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
    {
        let count = count as usize;
        progress.total += count;
        match status.as_str() {
            "done" => progress.imported += count,
            "duplicate" => progress.duplicates += count,
            "failed" => progress.failed += count,
            _ => {}
        }
    }
    progress.processed = progress.imported + progress.duplicates + progress.failed;
    on_progress(&progress);

    let items: Vec<Item> = conn
        .prepare(
            "SELECT id, source_path, target_path, status, content_hash FROM import_items
             WHERE import_id = ?1 AND status NOT IN ('done', 'duplicate', 'failed') ORDER BY id",
        )?
        .query_map([import_id], |r| {
            Ok(Item {
                id: r.get(0)?,
                source: PathBuf::from(r.get::<_, String>(1)?),
                target: r.get(2)?,
                status: r.get(3)?,
                content_hash: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    for mut item in items {
        if cancel.load(Ordering::SeqCst) {
            return stop(conn, import_id, ImportOutcome::Interrupted);
        }
        progress.current_file = item
            .source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        progress.media_id = None;
        match process_item(conn, &lib, &options, import_id, &mut item) {
            Ok(media_id) => {
                progress.imported += 1;
                progress.media_id = Some(media_id);
            }
            Err(ItemError::SourceMissing(path)) => {
                stop(conn, import_id, ImportOutcome::Interrupted)?;
                return Err(ImportError::SourceUnavailable(path));
            }
            Err(ItemError::Fatal(e)) => {
                stop(conn, import_id, ImportOutcome::Interrupted)?;
                return Err(e);
            }
            Err(ItemError::File(message)) => {
                progress.failed += 1;
                conn.execute(
                    "UPDATE import_items SET status = 'failed', error = ?2, updated_at = ?3
                     WHERE id = ?1",
                    params![item.id, message, now_utc()],
                )?;
            }
        }
        progress.processed += 1;
        on_progress(&progress);
    }
    stop(conn, import_id, ImportOutcome::Done)
}

fn stop(
    conn: &Connection,
    import_id: i64,
    outcome: ImportOutcome,
) -> Result<ImportOutcome, ImportError> {
    match outcome {
        ImportOutcome::Done => conn.execute(
            "UPDATE imports SET status = 'done', finished_at = ?2 WHERE id = ?1",
            params![import_id, now_utc()],
        )?,
        ImportOutcome::Interrupted => conn.execute(
            "UPDATE imports SET status = 'interrupted' WHERE id = ?1",
            [import_id],
        )?,
    };
    Ok(outcome)
}

enum ItemError {
    /// La source a disparu : on arrête tout, l'import reprendra plus tard.
    SourceMissing(PathBuf),
    /// Ce fichier échoue, les autres continuent.
    File(String),
    /// Erreur de base : on arrête.
    Fatal(ImportError),
}

impl From<rusqlite::Error> for ItemError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Fatal(e.into())
    }
}

impl From<LibraryError> for ItemError {
    fn from(e: LibraryError) -> Self {
        Self::Fatal(e.into())
    }
}

fn set_status(conn: &Connection, item: &mut Item, status: &str) -> Result<(), ItemError> {
    conn.execute(
        "UPDATE import_items SET status = ?2, content_hash = ?3, target_path = ?4, updated_at = ?5
         WHERE id = ?1",
        params![item.id, status, item.content_hash, item.target, now_utc()],
    )?;
    item.status = status.to_owned();
    Ok(())
}

fn process_item(
    conn: &mut Connection,
    lib: &Library,
    options: &StoredOptions,
    import_id: i64,
    item: &mut Item,
) -> Result<i64, ItemError> {
    let file_err = |e: &dyn std::fmt::Display| ItemError::File(e.to_string());

    if item.status == "pending" {
        if !item.source.is_file() {
            return Err(ItemError::SourceMissing(item.source.clone()));
        }
        let hash = copy_to_library(conn, lib, item)?;
        item.content_hash = Some(hash);
        set_status(conn, item, "verified")?;
    }

    let target = paths::to_absolute(&lib.root_path, &item.target).map_err(|e| file_err(&e))?;
    let hash = item
        .content_hash
        .clone()
        .ok_or_else(|| ItemError::File("somme de contrôle absente".into()))?;

    let mut backup_error = None;
    if item.status == "verified" {
        if let (true, Some(backup_root)) = (options.backup, &lib.backup_path)
            && let Err(e) = backup(backup_root, &item.target, &target, &hash)
        {
            backup_error = Some(format!("sauvegarde : {e}"));
        }
        if backup_error.is_none() {
            set_status(conn, item, "backed_up")?;
        }
    }

    // Les métadonnées sont relues sur la copie vérifiée, pas sur la source.
    let format = format_of(&target).ok_or_else(|| ItemError::File("format inconnu".into()))?;
    let file = scan::describe(&target, format).map_err(|e| file_err(&e))?;
    write_sidecar(&target, format, &item.source, options).map_err(|e| file_err(&e))?;
    let media_id = record_media(
        conn,
        lib,
        options,
        import_id,
        item,
        &file,
        &hash,
        backup_error,
    )?;
    Ok(media_id)
}

/// Copie vérifiée vers la bibliothèque ; gère la reprise quand le fichier
/// final existe déjà (coupure entre la copie et l'écriture en base).
fn copy_to_library(conn: &Connection, lib: &Library, item: &mut Item) -> Result<String, ItemError> {
    let file_err = |e: &dyn std::fmt::Display| ItemError::File(e.to_string());
    let mut target = paths::to_absolute(&lib.root_path, &item.target).map_err(|e| file_err(&e))?;
    if target.exists() {
        let source_hash = copy::hash_file(&item.source).map_err(|e| file_err(&e))?;
        if copy::hash_file(&target).map_err(|e| file_err(&e))? == source_hash {
            return Ok(source_hash);
        }
        // Un autre fichier occupe la place : on choisit un nouveau nom.
        let (folder, name) = item.target.rsplit_once('/').unwrap_or(("", &item.target));
        let mut taken = HashSet::new();
        item.target = unique_target(&lib.root_path, folder, name, &mut taken);
        set_status(conn, item, "pending")?;
        target = paths::to_absolute(&lib.root_path, &item.target).map_err(|e| file_err(&e))?;
    }
    match copy::copy_verified(&item.source, &target) {
        Ok(hash) => Ok(hash),
        Err(CopyError::Read { .. }) if !item.source.is_file() => {
            Err(ItemError::SourceMissing(item.source.clone()))
        }
        Err(e) => Err(file_err(&e)),
    }
}

fn backup(root: &Path, relative: &str, copy: &Path, hash: &str) -> Result<(), String> {
    let target = paths::to_absolute(root, relative).map_err(|e| e.to_string())?;
    if target.exists() {
        let existing = copy::hash_file(&target).map_err(|e| e.to_string())?;
        return if existing == hash {
            Ok(())
        } else {
            Err(format!(
                "{} existe déjà avec un autre contenu",
                target.display()
            ))
        };
    }
    let copied = copy::copy_verified(copy, &target).map_err(|e| e.to_string())?;
    if copied == hash {
        Ok(())
    } else {
        Err("somme de contrôle différente".into())
    }
}

/// Nommage validé (décision 0002) : `IMG.xmp` pour les RAW, `IMG.JPG.xmp` sinon.
pub fn sidecar_naming(format: MediaFormat) -> SidecarNaming {
    match format {
        MediaFormat::Raw => SidecarNaming::ReplaceExtension,
        MediaFormat::Image => SidecarNaming::AppendExtension,
    }
}

fn write_sidecar(
    target: &Path,
    format: MediaFormat,
    source: &Path,
    options: &StoredOptions,
) -> Result<(), xmp::XmpError> {
    let path = xmp::sidecar_path(target, sidecar_naming(format));
    // Un XMP déjà présent (autre logiciel, reprise) garde ses notes et mots-clés.
    let mut data = xmp::read_sidecar(&path)?.unwrap_or_default();
    let mut keywords: BTreeSet<String> = data.keywords.drain(..).collect();
    keywords.extend(options.keywords.iter().cloned());
    data = SidecarData {
        keywords: keywords.into_iter().collect(),
        creator: options.creator.clone().or(data.creator),
        rights: options.rights.clone().or(data.rights),
        original_file_name: source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .or(data.original_file_name),
        ..data
    };
    xmp::write_sidecar(&path, &data)
}

#[allow(clippy::too_many_arguments)]
fn record_media(
    conn: &mut Connection,
    lib: &Library,
    options: &StoredOptions,
    import_id: i64,
    item: &mut Item,
    file: &SourceFile,
    hash: &str,
    backup_error: Option<String>,
) -> Result<i64, ItemError> {
    let m: &MediaMetadata = &file.metadata;
    let (folder, file_name) = item.target.rsplit_once('/').unwrap_or(("", &item.target));
    let sidecar = xmp::sidecar_path(Path::new(&item.target), sidecar_naming(file.format))
        .to_string_lossy()
        .replace('\\', "/");
    let original_name = item
        .source
        .file_name()
        .map(|n| paths::nfc(&n.to_string_lossy()))
        .unwrap_or_default();
    let album_name = if options.album_name.is_empty() {
        folder.rsplit('/').next().unwrap_or(folder).to_owned()
    } else {
        options.album_name.clone()
    };
    // L'empreinte de la source fait foi pour reconnaître les doublons au prochain import.
    let fingerprint: String = conn.query_row(
        "SELECT fingerprint FROM import_items WHERE id = ?1",
        [item.id],
        |r| r.get(0),
    )?;

    let tx = conn.transaction()?;
    let album_id =
        library::find_or_create_album(&tx, lib.id, &album_name, folder, Some(file.day()))?;
    let camera_id = match (&m.make, &m.model) {
        (Some(make), Some(model)) => Some(library::find_or_create_gear(
            &tx,
            GearKind::Body,
            make,
            model,
            m.body_serial.as_deref(),
        )?),
        _ => None,
    };
    let lens_id = match &m.lens {
        Some(lens) => Some(library::find_or_create_gear(
            &tx,
            GearKind::Lens,
            "",
            lens,
            None,
        )?),
        None => None,
    };
    let now = now_utc();
    tx.execute(
        "INSERT INTO media (library_id, album_id, kind, path, sidecar_path, original_name,
                            file_size, fingerprint, content_hash, captured_at, utc_offset,
                            width, height, orientation, lat, lon, location_source,
                            camera_id, lens_id, import_id, imported_at, updated_at)
         VALUES (?1, ?2, 'photo', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                 ?16, ?17, ?18, ?19, ?20, ?20)
         ON CONFLICT (library_id, path) DO NOTHING",
        params![
            lib.id,
            album_id,
            item.target,
            sidecar,
            original_name,
            file.size as i64,
            fingerprint,
            hash,
            file.captured_at,
            m.utc_offset,
            m.width,
            m.height,
            m.orientation,
            m.gps.as_ref().map(|g| g.latitude),
            m.gps.as_ref().map(|g| g.longitude),
            m.gps.as_ref().map(|_| "exif"),
            camera_id,
            lens_id,
            import_id,
            now
        ],
    )?;
    let media_id: i64 = tx.query_row(
        "SELECT id FROM media WHERE library_id = ?1 AND path = ?2",
        params![lib.id, item.target],
        |r| r.get(0),
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO media_exif (media_id, focal_length, focal_length_35mm, aperture,
                 shutter, iso, exposure_compensation, exposure_program, flash, body_serial, raw_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            media_id,
            m.focal_length,
            m.focal_length_35mm,
            m.aperture,
            m.shutter,
            m.iso,
            m.exposure_compensation,
            m.exposure_program,
            m.flash,
            m.body_serial,
            m.tags_json()
        ],
    )?;
    for word in &options.keywords {
        tx.execute(
            "INSERT INTO keywords (word) VALUES (?1) ON CONFLICT (word) DO NOTHING",
            [word],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO media_keywords (media_id, keyword_id)
             SELECT ?1, id FROM keywords WHERE word = ?2",
            params![media_id, word],
        )?;
    }
    tx.execute(
        "INSERT INTO media_search (rowid, album, file_name, original_name, keywords, place)
         VALUES (?1, ?2, ?3, ?4, ?5, '')",
        params![
            media_id,
            album_name,
            file_name,
            original_name,
            options.keywords.join(" ")
        ],
    )?;
    tx.execute(
        "UPDATE import_items SET status = 'done', media_id = ?2, error = ?3, updated_at = ?4
         WHERE id = ?1",
        params![item.id, media_id, backup_error, now],
    )?;
    tx.execute(
        "UPDATE imports SET files_done = files_done + 1 WHERE id = ?1",
        [import_id],
    )?;
    tx.commit()?;
    item.status = "done".into();
    Ok(media_id)
}
