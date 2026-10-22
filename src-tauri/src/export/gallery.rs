//! Galerie web (cahier des charges, 4.6) : un site statique d'un album ou
//! d'une collection, généré dans un dossier choisi, à héberger où l'on veut.
//! Aucune ressource externe, aucun appel réseau : la page s'ouvre même d'un
//! double-clic. Les photos sont les aperçus retouchés, en JPEG sans aucune
//! métadonnée (ni GPS ni données personnelles), filigrane en option.
//!
//! Sélection client : le client coche ses photos dans son navigateur et
//! renvoie une ligne `[AD-XXXXXXXX: 3 12 27]` par courriel ou en fichier.
//! La base garde la liste numérotée de chaque galerie pour la relire.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use image::imageops::FilterType;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::ExportError;
use super::contact_sheet::SheetWatermark;
use super::presets;
use super::render::Rendered;
use super::watermark::{self, Anchor, Watermark, WatermarkContent};
use crate::db::now_utc;

/// Côté long des photos de la galerie et de leurs vignettes.
pub const PHOTO_SIDE: u32 = 2048;
pub const THUMB_SIDE: u32 = 640;
const JPEG_QUALITY: u8 = 85;
const CSS: &str = include_str!("../../assets/gallery/galerie.css");
const JS: &str = include_str!("../../assets/gallery/galerie.js");
/// Sans 0, O, 1, I ni L : un code se recopie sans erreur.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryRequest {
    pub library_id: i64,
    pub title: String,
    /// Client et séance, dates, photographe.
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub intro: String,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub allow_download: bool,
    #[serde(default)]
    pub selection: bool,
    #[serde(default)]
    pub watermark: SheetWatermark,
    #[serde(default)]
    pub album_id: Option<i64>,
    #[serde(default)]
    pub client_id: Option<i64>,
    pub media_ids: Vec<i64>,
    /// Dossier parent : la galerie va dans un sous-dossier à son nom.
    pub destination: PathBuf,
    /// Textes de la page, dans la langue de l'interface (`lang`, `photo`,
    /// `pick`, `send`…). Un texte absent laisse sa clé.
    #[serde(default)]
    pub texts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryResult {
    pub code: String,
    pub folder: PathBuf,
    pub photos: usize,
    /// Photos sans image (aperçu et original illisibles).
    pub missing: usize,
}

/// Sélection relue : code de la galerie et numéros cochés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSelection {
    pub code: String,
    pub numbers: Vec<u32>,
}

/// Galerie retrouvée pour une sélection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionMatch {
    pub gallery_id: i64,
    pub library_id: i64,
    pub code: String,
    pub title: String,
    pub client_id: Option<i64>,
    pub client_name: Option<String>,
    pub album_id: Option<i64>,
    pub media_ids: Vec<i64>,
    /// Numéros absents de la galerie (texte abîmé, autre galerie).
    pub unknown: Vec<u32>,
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// JSON placé dans la page : `<` échappé, pour ne jamais fermer la balise.
fn script_json(value: &serde_json::Value) -> String {
    value.to_string().replace('<', "\\u003c")
}

/// Code d'une galerie, `AD-` suivi de 8 caractères faciles à recopier.
fn new_code(conn: &Connection, seed: &str) -> Result<String, ExportError> {
    let base = format!(
        "{seed}|{}|{}|{:?}",
        now_utc(),
        std::process::id(),
        std::time::SystemTime::now()
    );
    for attempt in 0u32..64 {
        let hash = blake3::hash(format!("{base}|{attempt}").as_bytes());
        let code: String = hash.as_bytes()[..8]
            .iter()
            .map(|b| CODE_ALPHABET[usize::from(*b) % CODE_ALPHABET.len()] as char)
            .collect();
        let code = format!("AD-{code}");
        let taken: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM galleries WHERE code = ?1)",
                [&code],
                |r| r.get(0),
            )
            .map_err(|e| ExportError::Settings(e.to_string()))?;
        if !taken {
            return Ok(code);
        }
    }
    Err(ExportError::Settings("code de galerie introuvable".into()))
}

/// Sous-dossier libre au nom de la galerie : « Titre », « Titre (2) »…
fn free_folder(parent: &Path, title: &str) -> PathBuf {
    let base = presets::clean_name(title);
    let base = if base.is_empty() {
        "Galerie".to_owned()
    } else {
        base
    };
    let mut n = 1;
    loop {
        let name = if n == 1 {
            base.clone()
        } else {
            format!("{base} ({n})")
        };
        let path = parent.join(name);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

fn to_rendered(img: &DynamicImage) -> Rendered {
    let rgb = img.to_rgb8();
    Rendered {
        width: rgb.width(),
        height: rgb.height(),
        rgb: rgb.as_raw().iter().map(|v| f32::from(*v) / 255.0).collect(),
    }
}

fn encode_jpeg(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>, ExportError> {
    let (w, h) = (
        u16::try_from(width).map_err(|_| ExportError::TooLarge)?,
        u16::try_from(height).map_err(|_| ExportError::TooLarge)?,
    );
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY)
        .encode(rgb, w, h, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| ExportError::Encode(e.to_string()))?;
    Ok(out)
}

/// Filigrane centré, comme sur les planches contact.
fn mark_of(watermark: &SheetWatermark) -> Option<Watermark> {
    let content = match watermark {
        SheetWatermark::None => return None,
        SheetWatermark::Text(text) if text.trim().is_empty() => return None,
        SheetWatermark::Text(text) => WatermarkContent::Text {
            text: text.trim().to_owned(),
            dark: false,
        },
        SheetWatermark::Logo => WatermarkContent::StudioLogo,
    };
    Some(Watermark {
        content,
        anchor: Anchor::Center,
        size: 40.0,
        opacity: 35.0,
        margin: 0.0,
    })
}

/// JPEG de la photo, JPEG de la vignette, taille de la photo.
type PhotoFiles = (Vec<u8>, Vec<u8>, (u32, u32));

/// Photo de la galerie : réduite, filigranée, puis sa vignette.
fn photo_files(img: &DynamicImage, mark: Option<&Watermark>) -> Result<PhotoFiles, ExportError> {
    let img = if img.width().max(img.height()) > PHOTO_SIDE {
        img.resize(PHOTO_SIDE, PHOTO_SIDE, FilterType::Lanczos3)
    } else {
        img.clone()
    };
    let mut rendered = to_rendered(&img);
    if let Some(mark) = mark {
        watermark::apply(&mut rendered, mark)?;
    }
    let bytes = rendered.to_u8();
    let full = encode_jpeg(rendered.width, rendered.height, &bytes)?;
    let marked = image::RgbImage::from_raw(rendered.width, rendered.height, bytes)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| ExportError::Encode("image vide".into()))?;
    let small = marked
        .resize(THUMB_SIDE, THUMB_SIDE, FilterType::Triangle)
        .to_rgb8();
    let thumb = encode_jpeg(small.width(), small.height(), small.as_raw())?;
    Ok((full, thumb, (rendered.width, rendered.height)))
}

struct Tile {
    n: u32,
    file: String,
    width: u32,
    height: u32,
}

fn text<'a>(req: &'a GalleryRequest, key: &'a str) -> &'a str {
    req.texts.get(key).map_or(key, String::as_str)
}

/// La page : en-tête, grille, barre de sélection, visionneuse.
fn page(req: &GalleryRequest, code: &str, email: &str, tiles: &[Tile]) -> String {
    let t = |key: &str| escape(text(req, key));
    let pick_icon = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 20.5s-7.5-4.6-7.5-10.2A4.3 4.3 0 0 1 12 7.6a4.3 4.3 0 0 1 7.5 2.7c0 5.6-7.5 10.2-7.5 10.2z"/></svg>"#;
    let mut html = String::new();
    let lang = escape(req.texts.get("lang").map_or("fr", String::as_str));
    let theme = match req.theme {
        Theme::Light => "light",
        Theme::Dark => "dark",
    };
    let _ = write!(
        html,
        "<!doctype html>\n<html lang=\"{lang}\" data-theme=\"{theme}\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'self'; img-src 'self' blob: data:; style-src 'self'; script-src 'self'\">\n\
         <meta name=\"robots\" content=\"noindex\">\n\
         <title>{title}</title>\n<link rel=\"stylesheet\" href=\"galerie.css\">\n</head>\n<body>\n\
         <header class=\"header\">\n<h1>{title}</h1>\n",
        title = escape(req.title.trim()),
    );
    if !req.subtitle.trim().is_empty() {
        let _ = writeln!(
            html,
            "<p class=\"subtitle\">{}</p>",
            escape(req.subtitle.trim())
        );
    }
    if !req.intro.trim().is_empty() {
        let _ = writeln!(html, "<p class=\"intro\">{}</p>", escape(req.intro.trim()));
    }
    html.push_str("</header>\n<main class=\"grid\">\n");
    for tile in tiles {
        let label = escape(&text(req, "photo").replace("{n}", &tile.n.to_string()));
        let _ = write!(
            html,
            "<figure class=\"tile\" data-n=\"{n}\" data-full=\"photos/{file}\" data-file=\"{file}\">\
             <button type=\"button\" class=\"open\" aria-label=\"{label}\">\
             <img src=\"vignettes/{file}\" alt=\"{label}\" width=\"{w}\" height=\"{h}\" loading=\"lazy\"></button>",
            n = tile.n,
            file = tile.file,
            w = tile.width,
            h = tile.height,
        );
        if req.selection {
            let _ = write!(
                html,
                "<button type=\"button\" class=\"pick\" aria-pressed=\"false\" aria-label=\"{label}\">{pick_icon}</button>"
            );
        }
        let _ = writeln!(html, "<figcaption>{label}</figcaption></figure>");
    }
    html.push_str("</main>\n");
    let _ = writeln!(
        html,
        "<footer class=\"footer\">{}</footer>",
        escape(&text(req, "footer").replace("{code}", code))
    );
    if req.selection {
        let _ = write!(
            html,
            "<div class=\"bar\"><span class=\"count\" aria-live=\"polite\"></span>\
             <button type=\"button\" class=\"button primary open-send\" disabled>{send}</button></div>\n\
             <div class=\"send\" hidden><p>{hint}</p><div class=\"actions\">",
            send = t("send"),
            hint = t(if email.is_empty() {
                "sendHintFile"
            } else {
                "sendHint"
            }),
        );
        if !email.is_empty() {
            let _ = write!(
                html,
                "<a class=\"button primary mail\" href=\"#\">{}</a>",
                t("mail")
            );
        }
        let _ = writeln!(
            html,
            "<button type=\"button\" class=\"button file\">{}</button>\
             <button type=\"button\" class=\"button copy\">{}</button></div></div>",
            t("file"),
            t("copy")
        );
    }
    let _ = write!(
        html,
        "<div class=\"lightbox\" hidden role=\"dialog\" aria-modal=\"true\" aria-label=\"{title}\">\
         <div class=\"top\"><span class=\"label\"></span>",
        title = escape(req.title.trim())
    );
    if req.selection {
        let _ = write!(
            html,
            "<button type=\"button\" class=\"pick\" aria-pressed=\"false\">{pick_icon}</button>"
        );
    }
    if req.allow_download {
        let _ = write!(
            html,
            "<a class=\"button download\" href=\"#\">{}</a>",
            t("download")
        );
    }
    let _ = writeln!(
        html,
        "<button type=\"button\" class=\"button close\">{close}</button></div>\
         <div class=\"stage\"><button type=\"button\" class=\"nav prev\" aria-label=\"{prev}\">‹</button>\
         <img alt=\"\"><button type=\"button\" class=\"nav next\" aria-label=\"{next}\">›</button></div>\
         <div class=\"bottom\"></div></div>",
        close = t("close"),
        prev = t("previous"),
        next = t("next"),
    );
    let data = serde_json::json!({
        "code": code,
        "title": req.title.trim(),
        "email": email,
        "selection": req.selection,
        "texts": req.texts,
    });
    let _ = write!(
        html,
        "<script id=\"gallery-data\" type=\"application/json\">{}</script>\n\
         <script src=\"galerie.js\"></script>\n</body>\n</html>\n",
        script_json(&data)
    );
    html
}

/// Écrit la galerie. `image_of` donne l'image retouchée d'une photo ;
/// `progress(fait, total)` suit les photos. Rien n'est écrit hors du
/// nouveau sous-dossier, et les originaux ne sont jamais ouverts en écriture.
pub fn generate(
    conn: &Connection,
    req: &GalleryRequest,
    email: &str,
    image_of: impl Fn(i64) -> Option<DynamicImage>,
    mut progress: impl FnMut(usize, usize),
) -> Result<GalleryResult, ExportError> {
    let folder = free_folder(&req.destination, &req.title);
    std::fs::create_dir_all(folder.join("photos"))?;
    std::fs::create_dir_all(folder.join("vignettes"))?;
    let mark = mark_of(&req.watermark);
    let total = req.media_ids.len();
    let mut tiles = Vec::new();
    let mut kept = Vec::new();
    let mut missing = 0;
    for (i, &id) in req.media_ids.iter().enumerate() {
        match image_of(id) {
            Some(img) => {
                let n = u32::try_from(tiles.len() + 1).unwrap_or(u32::MAX);
                let file = format!("{n:04}.jpg");
                let (full, thumb, (width, height)) = photo_files(&img, mark.as_ref())?;
                std::fs::write(folder.join("photos").join(&file), full)?;
                std::fs::write(folder.join("vignettes").join(&file), thumb)?;
                tiles.push(Tile {
                    n,
                    file,
                    width,
                    height,
                });
                kept.push(id);
            }
            None => missing += 1,
        }
        progress(i + 1, total);
    }
    let code = new_code(conn, &req.title)?;
    std::fs::write(folder.join("galerie.css"), CSS)?;
    std::fs::write(folder.join("galerie.js"), JS)?;
    std::fs::write(folder.join("index.html"), page(req, &code, email, &tiles))?;
    record(conn, req, &code, &folder, &kept).map_err(|e| ExportError::Settings(e.to_string()))?;
    Ok(GalleryResult {
        code,
        folder,
        photos: kept.len(),
        missing,
    })
}

fn record(
    conn: &Connection,
    req: &GalleryRequest,
    code: &str,
    folder: &Path,
    media: &[i64],
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO galleries (library_id, code, title, album_id, client_id, folder, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            req.library_id,
            code,
            req.title.trim(),
            req.album_id,
            req.client_id,
            folder.to_string_lossy(),
            now_utc()
        ],
    )?;
    let id = conn.last_insert_rowid();
    let mut stmt = conn.prepare(
        "INSERT INTO gallery_items (gallery_id, position, media_id) VALUES (?1, ?2, ?3)",
    )?;
    for (i, media_id) in media.iter().enumerate() {
        stmt.execute(params![id, i as i64 + 1, media_id])?;
    }
    Ok(())
}

/// Relit la ligne `[AD-XXXXXXXX: 3 12 27]` d'un courriel ou d'un fichier.
/// Les retours à la ligne et les virgules ajoutés par une messagerie sont
/// tolérés.
pub fn parse_selection(text: &str) -> Option<ParsedSelection> {
    // Majuscules ASCII : mêmes positions, code retrouvé quelle que soit la casse.
    let text = text.to_ascii_uppercase();
    let start = text.find("[AD-")?;
    let rest = &text[start + 1..];
    let end = rest.find(']')?;
    let inside = &rest[..end];
    let (code, numbers) = inside.split_once(':')?;
    let code = code.trim().to_uppercase();
    let valid = code.len() == 11 && code[3..].bytes().all(|b| CODE_ALPHABET.contains(&b));
    if !valid {
        return None;
    }
    let mut numbers: Vec<u32> = numbers
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse().ok())
        .filter(|&n| n > 0)
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    Some(ParsedSelection { code, numbers })
}

/// Galerie : identifiant, bibliothèque, titre, client et son nom, album.
type GalleryRow = (i64, i64, String, Option<i64>, Option<String>, Option<i64>);

/// Galerie et photos d'une sélection relue ; `None` pour un code inconnu.
pub fn match_selection(
    conn: &Connection,
    selection: &ParsedSelection,
) -> rusqlite::Result<Option<SelectionMatch>> {
    let row: Option<GalleryRow> = conn
        .query_row(
            "SELECT g.id, g.library_id, g.title, g.client_id, c.name, g.album_id
               FROM galleries g LEFT JOIN clients c ON c.id = g.client_id
              WHERE g.code = ?1",
            [&selection.code],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()?;
    let Some((gallery_id, library_id, title, client_id, client_name, album_id)) = row else {
        return Ok(None);
    };
    let mut media_ids = Vec::new();
    let mut unknown = Vec::new();
    for &n in &selection.numbers {
        let media: Option<i64> = conn
            .query_row(
                "SELECT media_id FROM gallery_items WHERE gallery_id = ?1 AND position = ?2",
                params![gallery_id, n],
                |r| r.get(0),
            )
            .optional()?;
        match media {
            Some(id) => media_ids.push(id),
            None => unknown.push(n),
        }
    }
    Ok(Some(SelectionMatch {
        gallery_id,
        library_id,
        code: selection.code.clone(),
        title,
        client_id,
        client_name,
        album_id,
        media_ids,
        unknown,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_relue_malgre_la_messagerie() {
        let mail = "Bonjour,\n> Ma sélection : 3, 12, 27\n> [AD-7KQ3F9MZ: 3 12\n> 27 12]\nMerci";
        assert_eq!(
            parse_selection(mail),
            Some(ParsedSelection {
                code: "AD-7KQ3F9MZ".into(),
                numbers: vec![3, 12, 27],
            })
        );
        assert_eq!(
            parse_selection("[ad-7kq3f9mz: 1]").map(|s| s.code),
            Some("AD-7KQ3F9MZ".into())
        );
        // Code abîmé (0 et O exclus de l'alphabet), ou pas de ligne.
        assert_eq!(parse_selection("[AD-7KQ3F9M0: 1]"), None);
        assert_eq!(parse_selection("Photos 3 et 12"), None);
    }

    #[test]
    fn page_echappee_et_sans_ressource_externe() {
        let req = GalleryRequest {
            library_id: 1,
            title: "Mariage <Dupont>".into(),
            subtitle: "Famille Dupont · Mariage".into(),
            intro: "Bonjour !\nVoici vos photos.".into(),
            theme: Theme::Dark,
            allow_download: true,
            selection: true,
            watermark: SheetWatermark::None,
            album_id: None,
            client_id: None,
            media_ids: vec![],
            destination: PathBuf::new(),
            texts: [
                ("photo", "Photo {n}"),
                ("footer", "Code : {code}"),
                ("lang", "fr"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        };
        let tiles = [Tile {
            n: 1,
            file: "0001.jpg".into(),
            width: 640,
            height: 427,
        }];
        let html = page(&req, "AD-7KQ3F9MZ", "studio@example.com", &tiles);
        assert!(html.contains("<h1>Mariage &lt;Dupont&gt;</h1>"));
        assert!(html.contains("data-theme=\"dark\""));
        assert!(html.contains("Code : AD-7KQ3F9MZ"));
        assert!(html.contains("class=\"button primary mail\""));
        assert!(html.contains("class=\"button download\""));
        assert!(!html.contains("http://") && !html.contains("https://"));
        // Le titre dans le JSON ne ferme pas la balise script.
        assert!(!html.contains("\"Mariage <Dupont>\""));
    }
}
