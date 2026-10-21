//! Lecture des médias et des albums pour les grilles et la fiche d'informations,
//! et index du cache d'aperçus.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::filter::{MediaFilter, MediaSort};
use super::{LibraryError, copies, get_library, pairs, paths};
use crate::db::now_utc;
use crate::develop::lens::{self, Shot};
use crate::develop::settings::DevelopSettings;
use crate::develop::store::{self, ORIGINAL};
use crate::export::color::ColorSpace;
use crate::export::render::{RenderOptions, Resize, Sharpen, render_photo};
use crate::metadata::media::{MediaFormat, format_of};
use crate::raw::preview::{self, Generated, PreviewError, SIZES};
use image::RgbImage;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSummary {
    pub id: i64,
    pub album_id: i64,
    pub file_name: String,
    pub captured_at: Option<String>,
    pub rating: i64,
    pub flag: i64,
    pub color: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub orientation: i64,
    /// Les trois paliers d'aperçu sont-ils en cache ?
    pub has_preview: bool,
    /// Date de mise à la corbeille (vue Corbeille seulement).
    pub trashed_at: Option<String>,
    /// Fichiers de la pile RAW + JPEG (1 pour une photo seule).
    pub stack_size: i64,
    /// Pile dépliée : fichier du dessus, pour les fichiers du dessous.
    pub companion_of: Option<i64>,
    /// Copie virtuelle (identifiant négatif) : la photo dont elle est la copie.
    pub copy_of: Option<i64>,
    /// Copie virtuelle : son nom.
    pub version: Option<String>,
}

const SUMMARY_COLUMNS: &str = "m.id, m.album_id, m.path, m.captured_at, m.rating, m.flag, m.color,
     m.width, m.height, m.orientation,
     (SELECT count(*) FROM previews p WHERE p.media_id = m.id) = 3,
     m.trashed_at, m.companion_of";

fn summary(r: &rusqlite::Row<'_>, stack_size: i64) -> rusqlite::Result<MediaSummary> {
    let path: String = r.get(2)?;
    Ok(MediaSummary {
        id: r.get(0)?,
        album_id: r.get(1)?,
        file_name: path.rsplit('/').next().unwrap_or(&path).to_owned(),
        captured_at: r.get(3)?,
        rating: r.get(4)?,
        flag: r.get(5)?,
        color: r.get(6)?,
        width: r.get(7)?,
        height: r.get(8)?,
        orientation: r.get(9)?,
        has_preview: r.get(10)?,
        trashed_at: r.get(11)?,
        companion_of: r.get(12)?,
        stack_size,
        copy_of: None,
        version: None,
    })
}

/// Ce qu'affiche une grille : bibliothèque ou album, ou corbeille, filtrés et triés.
#[derive(Debug, Clone, Default)]
pub struct MediaQuery {
    pub album_id: Option<i64>,
    /// Corbeille : les plus récemment jetées d'abord, quel que soit le tri.
    pub trashed: bool,
    pub filter: MediaFilter,
    pub sort: MediaSort,
    /// Piles dépliées (fichier du dessus) : leurs autres fichiers suivent.
    pub expanded: Vec<i64>,
}

pub fn list_media(
    conn: &Connection,
    library_id: i64,
    query: &MediaQuery,
) -> Result<Vec<MediaSummary>, LibraryError> {
    let (mut clauses, mut values) = query.filter.to_sql();
    clauses.push(pairs::HEAD_SQL.to_owned());
    clauses.insert(0, "m.library_id = ?".to_owned());
    values.insert(0, library_id.into());
    clauses.push(
        if query.trashed {
            "m.state = 'trashed'"
        } else {
            "m.state = 'normal'"
        }
        .to_owned(),
    );
    if let Some(album_id) = query.album_id {
        clauses.push("m.album_id = ?".to_owned());
        values.push(album_id.into());
    }
    let order = if query.trashed {
        format!("m.trashed_at DESC, {}", query.sort.order_by())
    } else {
        query.sort.order_by().to_owned()
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {SUMMARY_COLUMNS}, {}
         FROM media m
         WHERE {}
         ORDER BY {order}",
        pairs::STACK_SIZE_SQL,
        clauses.join(" AND ")
    ))?;
    let heads: Vec<MediaSummary> = stmt
        .query_map(rusqlite::params_from_iter(values), |r| {
            summary(r, r.get(13)?)
        })?
        .collect::<Result<_, _>>()?;
    let rows = expand_piles(conn, query, heads)?;
    if query.trashed {
        return Ok(rows);
    }
    // Copies virtuelles juste après leur photo (marques partagées).
    let copies = copies::copies_by_media(conn, library_id)?;
    if copies.is_empty() {
        return Ok(rows);
    }
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let of = copies.get(&row.id).cloned().unwrap_or_default();
        let parent = row.clone();
        out.push(row);
        for c in of {
            out.push(MediaSummary {
                id: c.item_id,
                copy_of: Some(parent.id),
                version: Some(c.version),
                stack_size: 1,
                companion_of: None,
                ..parent.clone()
            });
        }
    }
    Ok(out)
}

/// Piles dépliées : les fichiers du dessous suivent celui du dessus.
fn expand_piles(
    conn: &Connection,
    query: &MediaQuery,
    heads: Vec<MediaSummary>,
) -> Result<Vec<MediaSummary>, LibraryError> {
    if query.expanded.is_empty() {
        return Ok(heads);
    }
    // Piles dépliées : les fichiers du dessous suivent celui du dessus.
    let mut below = conn.prepare(&format!(
        "SELECT {SUMMARY_COLUMNS} FROM media m
         WHERE m.companion_of = ?1 AND m.state = ?2 ORDER BY m.path"
    ))?;
    let state = if query.trashed { "trashed" } else { "normal" };
    let mut rows = Vec::with_capacity(heads.len());
    for head in heads {
        let open = head.stack_size > 1 && query.expanded.contains(&head.id);
        let id = head.id;
        rows.push(head);
        if open {
            let files: Vec<MediaSummary> = below
                .query_map(params![id, state], |r| summary(r, 1))?
                .collect::<Result<_, _>>()?;
            rows.extend(files);
        }
    }
    Ok(rows)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub id: i64,
    pub name: String,
    pub folder: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub count: i64,
    /// Couverture choisie, sinon la première photo de l'album.
    pub cover_media_id: Option<i64>,
    /// La couverture a-t-elle été choisie par l'utilisateur ?
    pub cover_chosen: bool,
    /// Saisie libre en V0.1.
    pub place: Option<String>,
}

pub fn list_albums(conn: &Connection, library_id: i64) -> Result<Vec<AlbumSummary>, LibraryError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT a.id, a.name, a.folder, a.start_date, a.end_date,
                (SELECT count(*) FROM media m WHERE m.album_id = a.id AND m.state = 'normal'
                   AND {head}),
                coalesce((SELECT m.id FROM media m
                           WHERE m.id = a.cover_media_id AND m.state = 'normal'),
                         (SELECT m.id FROM media m WHERE m.album_id = a.id AND m.state = 'normal'
                            AND {head}
                          ORDER BY m.captured_at, m.path LIMIT 1)),
                EXISTS (SELECT 1 FROM media m
                         WHERE m.id = a.cover_media_id AND m.state = 'normal'),
                a.place
         FROM albums a WHERE a.library_id = ?1
         ORDER BY a.start_date DESC, a.name",
        head = pairs::HEAD_SQL
    ))?;
    let rows = stmt
        .query_map([library_id], |r| {
            Ok(AlbumSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                folder: r.get(2)?,
                start_date: r.get(3)?,
                end_date: r.get(4)?,
                count: r.get(5)?,
                cover_media_id: r.get(6)?,
                cover_chosen: r.get(7)?,
                place: r.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Photo pouvant illustrer l'accueil.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub media_id: i64,
    pub album_name: String,
    pub captured_at: Option<String>,
}

/// Meilleures photos de la bibliothèque pour l'illustration de l'accueil :
/// retenues et mieux notées d'abord, puis les plus récentes, avec un aperçu
/// 2048 px en cache. Les rejetées sont exclues.
pub fn highlights(
    conn: &Connection,
    library_id: i64,
    limit: usize,
) -> Result<Vec<Highlight>, LibraryError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT m.id, a.name, m.captured_at FROM media m
         JOIN albums a ON a.id = m.album_id
         WHERE m.library_id = ?1 AND m.state = 'normal' AND m.flag >= 0 AND {}
           AND EXISTS (SELECT 1 FROM previews p WHERE p.media_id = m.id AND p.size = 2048)
         ORDER BY m.flag DESC, m.rating DESC, m.captured_at DESC
         LIMIT ?2",
        pairs::HEAD_SQL
    ))?;
    let rows = stmt
        .query_map(params![library_id, limit as i64], |r| {
            Ok(Highlight {
                media_id: r.get(0)?,
                album_name: r.get(1)?,
                captured_at: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Fiche d'informations d'une photo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDetails {
    pub id: i64,
    pub file_name: String,
    pub original_name: String,
    pub path: String,
    pub captured_at: Option<String>,
    pub utc_offset: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<f64>,
    pub focal_length_35mm: Option<i64>,
    pub aperture: Option<f64>,
    pub shutter: Option<f64>,
    pub iso: Option<f64>,
    pub exposure_compensation: Option<f64>,
    pub keywords: Vec<String>,
    /// Pile RAW + JPEG : noms des fichiers, le dessus d'abord.
    pub stack_files: Vec<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// « exif », « gpx », « phone » ou « manual ».
    pub location_source: Option<String>,
    /// Lieu habité le plus proche (géocodage hors ligne).
    pub city: Option<String>,
    pub region: Option<String>,
    /// Code ISO du pays ; vide si aucun lieu habité à proximité.
    pub country: Option<String>,
    /// Heure écrite par le boîtier, si elle a été corrigée d'après le lieu.
    pub captured_at_camera: Option<String>,
}

/// Séparateur des mots-clés dans `group_concat` (caractère de contrôle US).
const KEYWORD_SEPARATOR: char = 31u8 as char;

pub fn media_details(conn: &Connection, id: i64) -> Result<Option<MediaDetails>, LibraryError> {
    // Copie virtuelle : la fiche de sa photo.
    let Some((id, _)) = copies::resolve(conn, id)? else {
        return Ok(None);
    };
    conn.query_row(
        "SELECT m.id, m.path, m.original_name, m.captured_at, m.utc_offset, m.width, m.height,
                    trim(coalesce(c.make, '') || ' ' || coalesce(c.model, '')), l.model,
                    e.focal_length, e.focal_length_35mm, e.aperture, e.shutter, e.iso,
                    e.exposure_compensation,
                    (SELECT group_concat(k.word, char(31)) FROM media_keywords mk
                       JOIN keywords k ON k.id = mk.keyword_id WHERE mk.media_id = m.id),
                    m.lat, m.lon, m.location_source, m.city, m.region, m.country,
                    m.captured_at_camera
             FROM media m
             LEFT JOIN gear c ON c.id = m.camera_id
             LEFT JOIN gear l ON l.id = m.lens_id
             LEFT JOIN media_exif e ON e.media_id = m.id
             WHERE m.id = ?1",
        [id],
        |r| {
            let path: String = r.get(1)?;
            Ok(MediaDetails {
                id: r.get(0)?,
                file_name: path.rsplit('/').next().unwrap_or(&path).to_owned(),
                path,
                original_name: r.get(2)?,
                captured_at: r.get(3)?,
                utc_offset: r.get(4)?,
                width: r.get(5)?,
                height: r.get(6)?,
                camera: r.get::<_, Option<String>>(7)?.filter(|s| !s.is_empty()),
                lens: r.get(8)?,
                focal_length: r.get(9)?,
                focal_length_35mm: r.get(10)?,
                aperture: r.get(11)?,
                shutter: r.get(12)?,
                iso: r.get(13)?,
                exposure_compensation: r.get(14)?,
                keywords: r
                    .get::<_, Option<String>>(15)?
                    .map(|k| k.split(KEYWORD_SEPARATOR).map(str::to_owned).collect())
                    .unwrap_or_default(),
                stack_files: Vec::new(),
                latitude: r.get(16)?,
                longitude: r.get(17)?,
                location_source: r.get(18)?,
                city: r.get(19)?,
                region: r.get(20)?,
                country: r.get(21)?,
                captured_at_camera: r.get(22)?,
            })
        },
    )
    .optional()?
    .map(|mut details| -> Result<MediaDetails, LibraryError> {
        let state: String =
            conn.query_row("SELECT state FROM media WHERE id = ?1", [id], |r| r.get(0))?;
        let ids = pairs::members(conn, id, Some(&state))?;
        if ids.len() > 1 {
            for member in ids {
                let path: String =
                    conn.query_row("SELECT path FROM media WHERE id = ?1", [member], |r| {
                        r.get(0)
                    })?;
                details
                    .stack_files
                    .push(path.rsplit('/').next().unwrap_or(&path).to_owned());
            }
        }
        Ok(details)
    })
    .transpose()
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Path(#[from] paths::PathError),
    #[error(transparent)]
    Preview(#[from] PreviewError),
    #[error("média {0} introuvable")]
    NotFound(i64),
    #[error("bibliothèque hors ligne : {0}")]
    Offline(PathBuf),
    #[error("format sans aperçu : {0}")]
    UnknownFormat(String),
    #[error("rendu de la retouche impossible : {0}")]
    Render(String),
}

/// Génère et indexe les aperçus d'un média, depuis l'original dans la bibliothèque.
pub fn generate_previews(
    conn: &Connection,
    cache_root: &Path,
    item_id: i64,
) -> Result<Vec<Generated>, CacheError> {
    let (media_id, version) =
        copies::resolve(conn, item_id)?.ok_or(CacheError::NotFound(item_id))?;
    let (library_id, path, orientation): (i64, String, u8) = conn
        .query_row(
            "SELECT library_id, path, orientation FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or(CacheError::NotFound(media_id))?;
    let lib = get_library(conn, library_id)?;
    if !lib.is_online {
        return Err(CacheError::Offline(lib.root_path));
    }
    let abs = paths::to_absolute(&lib.root_path, &path)?;
    let format = format_of(&abs).ok_or_else(|| CacheError::UnknownFormat(path.clone()))?;
    // Copie virtuelle : rendue d'après ses propres réglages, sans ligne dans
    // `previews` (elle n'est pas un média).
    if item_id != media_id {
        let settings =
            store::load(conn, media_id, &version).map_err(|e| CacheError::Render(e.to_string()))?;
        return rendered_previews(&abs, &settings, cache_root, item_id);
    }
    // Photo retouchée : l'aperçu montre la retouche, rendue par le moteur.
    if format == MediaFormat::Raw
        && let Some(settings) = retouched(conn, media_id)
    {
        let generated = rendered_previews(&abs, &settings, cache_root, media_id)?;
        record(conn, media_id, &generated, "rendered")?;
        return Ok(generated);
    }
    let mut generated = preview::generate(&abs, format, orientation, cache_root, media_id)?;
    let mut source = match format {
        MediaFormat::Raw => "embedded",
        MediaFormat::Image => "decoded",
    };
    // Aperçu intégré plus petit que le grand palier (Sony, Panasonic) : le
    // JPEG du boîtier, s'il est dans la pile, donne des aperçus nets.
    let largest = generated
        .iter()
        .map(|g| g.width.max(g.height))
        .max()
        .unwrap_or(0);
    let top_size = SIZES.iter().copied().max().unwrap_or(0);
    if format == MediaFormat::Raw
        && largest < top_size
        && let Some((jpeg, jpeg_orientation, jpeg_long_side)) =
            companion_jpeg(conn, &lib.root_path, media_id)?
        && jpeg_long_side > i64::from(largest)
    {
        generated = preview::generate(
            &jpeg,
            MediaFormat::Image,
            jpeg_orientation,
            cache_root,
            media_id,
        )?;
        source = "decoded";
    }
    record(conn, media_id, &generated, source)?;
    Ok(generated)
}

/// Note les aperçus produits.
fn record(
    conn: &Connection,
    media_id: i64,
    generated: &[Generated],
    source: &str,
) -> Result<(), CacheError> {
    let now = now_utc();
    for g in generated {
        conn.execute(
            "INSERT OR REPLACE INTO previews
               (media_id, size, width, height, bytes, source, created_at, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                media_id,
                g.size,
                g.width,
                g.height,
                g.bytes as i64,
                source,
                now
            ],
        )?;
    }
    Ok(())
}

/// Réglages de la version originale d'une photo retouchée, ou rien.
pub fn retouched(conn: &Connection, media_id: i64) -> Option<DevelopSettings> {
    store::load(conn, media_id, ORIGINAL)
        .ok()
        .filter(|s| !s.is_neutral())
}

/// Aperçus rendus d'après la retouche (2048 px, puis les paliers plus petits).
fn rendered_previews(
    path: &Path,
    settings: &DevelopSettings,
    cache_root: &Path,
    media_id: i64,
) -> Result<Vec<Generated>, CacheError> {
    let largest = SIZES.iter().copied().max().unwrap_or(2048);
    let mut raw = crate::raw::RawFile::open(path).map_err(PreviewError::from)?;
    let profile = lens::find_profile(&Shot::from(&raw.info()));
    let options = RenderOptions {
        resize: Resize::LongSide(largest),
        sharpen: Sharpen::Screen,
        color: ColorSpace::Srgb,
    };
    let img = render_photo(&mut raw, settings, profile.as_ref(), options, None)
        .map_err(|e| CacheError::Render(e.to_string()))?;
    let rgb = RgbImage::from_raw(img.width, img.height, img.to_u8())
        .ok_or_else(|| CacheError::Render("dimensions".into()))?;
    Ok(preview::generate_from(rgb, cache_root, media_id)?)
}

/// Photos dont la retouche est plus récente que les aperçus.
pub fn stale_previews(conn: &Connection) -> Result<Vec<i64>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT e.media_id FROM edits e JOIN media m ON m.id = e.media_id
         WHERE e.name = '' AND m.state = 'normal'
           AND e.updated_at >= coalesce(
             (SELECT min(p.created_at) FROM previews p WHERE p.media_id = e.media_id), '')",
    )?;
    let ids = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// La retouche d'une photo est-elle plus récente que ses aperçus ?
pub fn is_stale(conn: &Connection, media_id: i64) -> bool {
    conn.query_row(
        "SELECT e.updated_at >= coalesce(
           (SELECT min(p.created_at) FROM previews p WHERE p.media_id = e.media_id), '')
         FROM edits e WHERE e.media_id = ?1 AND e.name = ''",
        [media_id],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

/// JPEG du boîtier dans la pile RAW + JPEG d'une photo : chemin absolu,
/// orientation et plus grand côté.
fn companion_jpeg(
    conn: &Connection,
    root: &Path,
    media_id: i64,
) -> Result<Option<(PathBuf, u8, i64)>, CacheError> {
    let state: String =
        conn.query_row("SELECT state FROM media WHERE id = ?1", [media_id], |r| {
            r.get(0)
        })?;
    for member in pairs::members(conn, media_id, Some(&state))? {
        if member == media_id {
            continue;
        }
        let (path, orientation, long_side): (String, u8, Option<i64>) = conn.query_row(
            "SELECT path, orientation, max(coalesce(width, 0), coalesce(height, 0))
             FROM media WHERE id = ?1",
            [member],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let is_jpeg = Path::new(&path)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"));
        if is_jpeg {
            let abs = paths::to_absolute(root, &path)?;
            // Sans dimensions en EXIF, l'en-tête du JPEG les donne.
            let long_side = match long_side {
                Some(n) if n > 0 => n,
                _ => image::image_dimensions(&abs).map_or(0, |(w, h)| i64::from(w.max(h))),
            };
            return Ok(Some((abs, orientation, long_side)));
        }
    }
    Ok(None)
}

/// JPEG pleine définition pour le zoom 100 % : le plus grand aperçu intégré
/// au RAW (extrait sans décodage), ou le fichier lui-même pour un JPEG.
/// L'original est ouvert en lecture seule. `None` si la bibliothèque est
/// débranchée ou si le format n'en fournit pas : l'interface garde alors
/// l'aperçu 2048 px. L'orientation n'est pas appliquée (voir `orientation`).
pub fn full_size_jpeg(conn: &Connection, media_id: i64) -> Result<Option<Vec<u8>>, CacheError> {
    // Copie virtuelle : l'aperçu rendu de 2048 px suffit.
    if copies::is_copy(media_id) {
        return Ok(None);
    }
    let (library_id, path): (i64, String) = conn
        .query_row(
            "SELECT library_id, path FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(CacheError::NotFound(media_id))?;
    let lib = get_library(conn, library_id)?;
    if !lib.is_online {
        return Ok(None);
    }
    let abs = paths::to_absolute(&lib.root_path, &path)?;
    let is_jpeg_file = abs
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"));
    match format_of(&abs) {
        // Photo retouchée : l'aperçu intégré ne la montre pas, l'interface
        // garde l'aperçu rendu de 2048 px.
        Some(MediaFormat::Raw) if retouched(conn, media_id).is_some() => Ok(None),
        Some(MediaFormat::Raw) => {
            // Le JPEG du boîtier a la définition du capteur, au moins autant
            // que l'aperçu intégré : on le préfère quand il est dans la pile.
            if let Some((jpeg, _, _)) = companion_jpeg(conn, &lib.root_path, media_id)? {
                return std::fs::read(&jpeg)
                    .map(Some)
                    .map_err(|source| PreviewError::Io { path: jpeg, source }.into());
            }
            let mut raw = crate::raw::RawFile::open(&abs).map_err(PreviewError::from)?;
            let preview = raw.preview().map_err(PreviewError::from)?;
            Ok((preview.format == crate::raw::PreviewFormat::Jpeg).then_some(preview.data))
        }
        Some(MediaFormat::Image) if is_jpeg_file => std::fs::read(&abs)
            .map(Some)
            .map_err(|source| PreviewError::Io { path: abs, source }.into()),
        _ => Ok(None),
    }
}

/// Médias dont un palier manque (import interrompu, cache vidé).
pub fn missing_previews(conn: &Connection) -> Result<Vec<i64>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT m.id FROM media m
         WHERE m.state = 'normal'
           AND (SELECT count(*) FROM previews p WHERE p.media_id = m.id) < ?1
         ORDER BY m.imported_at DESC",
    )?;
    let ids = stmt
        .query_map([SIZES.len() as i64], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Fichier d'aperçu en cache, s'il existe.
pub fn cached_preview(cache_root: &Path, media_id: i64, size: u32) -> Option<PathBuf> {
    let path = preview::cache_path(cache_root, media_id, size);
    path.is_file().then_some(path)
}
