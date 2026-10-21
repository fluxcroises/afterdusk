//! Export de l'inventaire (cahier des charges, 4.8) : CSV pour un tableur,
//! PDF pour l'assurance ou l'archive. L'interface fournit les textes déjà
//! traduits et mis en forme ; le cœur écrit le fichier et va chercher la
//! photo de chaque appareil. Le fichier est écrit à côté puis renommé : un
//! export interrompu ne laisse pas de fichier à moitié écrit.

use std::path::Path;

use image::DynamicImage;
use serde::Deserialize;

use super::GearError;
pub use crate::pdf::Table;
use crate::pdf::{Layout, Orientation, PageSize, Paper, gray};

const MARGIN: f32 = 48.0;
const PHOTO_W: f32 = 150.0;
const PHOTO_H: f32 = 100.0;
/// Logo du studio, en haut à droite de la première page.
const LOGO_W: f32 = 120.0;
const LOGO_H: f32 = 40.0;

/// Fiche d'un appareil : champs renseignés, puis son journal d'entretien.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub gear_id: i64,
    pub title: String,
    pub subtitle: String,
    pub fields: Vec<(String, String)>,
    pub journal: Option<Table>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryPdf {
    pub title: String,
    pub subtitle: String,
    pub table: Table,
    pub sections: Vec<Section>,
    /// Pied de page, `{page}` et `{pages}` remplacés.
    pub footer: String,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), GearError> {
    crate::pdf::write_atomic(path, bytes).map_err(|e| GearError::Export(e.to_string()))
}

fn csv_field(value: &str, delimiter: char) -> String {
    if value.contains([delimiter, '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// CSV en UTF-8 avec BOM (Excel le lit alors correctement), lignes CRLF ;
/// le séparateur suit la langue (point-virgule là où la virgule est décimale).
pub fn csv(table: &Table, delimiter: char) -> String {
    let line = |cells: &[String]| {
        cells
            .iter()
            .map(|c| csv_field(c, delimiter))
            .collect::<Vec<_>>()
            .join(&delimiter.to_string())
    };
    let mut out = String::from('\u{feff}');
    for row in std::iter::once(&table.headers).chain(&table.rows) {
        out.push_str(&line(row));
        out.push_str("\r\n");
    }
    out
}

pub fn write_csv(path: &Path, table: &Table, delimiter: char) -> Result<(), GearError> {
    write_atomic(path, csv(table, delimiter).as_bytes())
}

/// PDF de l'inventaire : tableau récapitulatif, puis une fiche par appareil.
/// `photo` donne l'image d'un appareil (JPEG ou PNG), s'il en a une ; `logo`
/// est celui du studio (Réglages > Identité).
pub fn pdf(
    doc_in: &InventoryPdf,
    logo: Option<&DynamicImage>,
    photo: impl Fn(i64) -> Option<Vec<u8>>,
) -> Result<Vec<u8>, GearError> {
    let page = PageSize::new(Paper::A4, Orientation::Portrait);
    let mut l = Layout::new(&doc_in.title, page, MARGIN)
        .ok_or_else(|| GearError::Export("police illisible".into()))?;

    l.ensure(60.0);
    if let Some(logo) = logo {
        let right = page.w - MARGIN;
        let _ = l.image(logo, right - LOGO_W, MARGIN - 6.0, LOGO_W, LOGO_H, false);
    }
    let title_w = l.inner_width() - if logo.is_some() { LOGO_W + 12.0 } else { 0.0 };
    let title = l.fit(&doc_in.title, 18.0, title_w);
    l.text(MARGIN, l.y, 18.0, gray(0.05), true, &title);
    l.y += 26.0;
    l.text(MARGIN, l.y, 9.5, gray(0.4), false, &doc_in.subtitle);
    l.y += 26.0;
    l.table(&doc_in.table, 8.5);

    for s in &doc_in.sections {
        let fields_h = s.fields.len() as f32 * 15.0;
        // Titre, photo ou champs, et au moins deux lignes du journal ensemble.
        l.ensure(48.0 + fields_h.max(PHOTO_H) + 40.0);
        l.y += 14.0;
        let sep = l.y;
        l.rule(sep, 0.75);
        l.y += 14.0;
        let top = l.y;
        l.text(MARGIN, top, 13.0, gray(0.05), true, &s.title);
        l.text(MARGIN, top + 18.0, 9.0, gray(0.4), false, &s.subtitle);
        l.y += 38.0;
        let block = l.y;
        let has_photo = photo(s.gear_id)
            .and_then(|b| image::load_from_memory(&b).ok())
            .and_then(|img| l.image(&img, MARGIN, block, PHOTO_W, PHOTO_H, false))
            .is_some();
        let x = if has_photo {
            MARGIN + PHOTO_W + 18.0
        } else {
            MARGIN
        };
        let label_w = 110.0;
        let value_w = page.w - MARGIN - x - label_w;
        for (i, (label, value)) in s.fields.iter().enumerate() {
            let t = block + i as f32 * 15.0;
            let lab = l.fit(label, 9.0, label_w - 8.0);
            l.text(x, t, 9.0, gray(0.4), false, &lab);
            let val = l.fit(value, 9.0, value_w);
            l.text(x + label_w, t, 9.0, gray(0.1), false, &val);
        }
        l.y = block + fields_h.max(if has_photo { PHOTO_H } else { 0.0 }) + 12.0;
        if let Some(journal) = &s.journal {
            l.table(journal, 8.5);
        }
    }
    l.footer(&doc_in.footer);
    Ok(l.finish())
}

/// Image d'un appareil pour le PDF : celle déposée sur la fiche, sinon
/// l'aperçu en cache de la photo choisie dans la bibliothèque.
pub fn gear_photo(
    conn: &rusqlite::Connection,
    data_dir: &Path,
    cache_root: &Path,
    id: i64,
) -> Option<Vec<u8>> {
    if let Ok(Some(bytes)) = super::photo_file(conn, data_dir, id) {
        return Some(bytes);
    }
    let media: i64 = conn
        .query_row("SELECT photo_media_id FROM gear WHERE id = ?1", [id], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .ok()
        .flatten()?;
    [1024, 256]
        .into_iter()
        .find_map(|size| crate::library::media::cached_preview(cache_root, media, size))
        .and_then(|p| std::fs::read(p).ok())
}

pub fn write_pdf(
    path: &Path,
    doc: &InventoryPdf,
    logo: Option<&DynamicImage>,
    photo: impl Fn(i64) -> Option<Vec<u8>>,
) -> Result<(), GearError> {
    write_atomic(path, &pdf(doc, logo, photo)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table {
            headers: vec!["Type".into(), "Appareil".into(), "Notes".into()],
            rows: vec![
                vec![
                    "Boîtier".into(),
                    "Nikon Z 6".into(),
                    "écran rayé; à revoir".into(),
                ],
                vec!["Objectif".into(), "24-70 \"S\"".into(), "".into()],
            ],
            widths: vec![],
        }
    }

    #[test]
    fn csv_avec_bom_guillemets_et_separateur() {
        let out = csv(&table(), ';');
        assert!(out.starts_with('\u{feff}'));
        let lines: Vec<&str> = out.trim_start_matches('\u{feff}').split("\r\n").collect();
        assert_eq!(lines[0], "Type;Appareil;Notes");
        assert_eq!(lines[1], "Boîtier;Nikon Z 6;\"écran rayé; à revoir\"");
        assert_eq!(lines[2], "Objectif;\"24-70 \"\"S\"\"\";");
    }

    #[test]
    fn pdf_sur_plusieurs_pages() {
        let mut t = table();
        for i in 0..80 {
            t.rows
                .push(vec!["Filtre".into(), format!("ND {i}"), "—".into()]);
        }
        let doc = InventoryPdf {
            title: "Inventaire du matériel".into(),
            subtitle: "Photos · 21 oct. 2026".into(),
            table: t.clone(),
            sections: vec![Section {
                gear_id: 1,
                title: "Nikon Z 6".into(),
                subtitle: "Boîtier".into(),
                fields: vec![("Numéro de série".into(), "6035906".into())],
                journal: Some(t),
            }],
            footer: "Page {page} sur {pages}".into(),
        };
        let jpeg = {
            let img = image::RgbImage::from_pixel(60, 40, image::Rgb([200, 120, 40]));
            let mut out = std::io::Cursor::new(Vec::new());
            img.write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
            out.into_inner()
        };
        let logo = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            90,
            30,
            image::Rgba([20, 20, 20, 200]),
        ));
        let bytes = pdf(&doc, Some(&logo), |_| Some(jpeg.clone())).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        // Relu par printpdf : 160 lignes de tableau ne tiennent pas sur une page.
        let parsed =
            printpdf::PdfDocument::parse(&bytes, &Default::default(), &mut Vec::new()).unwrap();
        assert!(parsed.pages.len() >= 4, "{} pages", parsed.pages.len());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inventaire.pdf");
        write_pdf(&path, &doc, None, |_| None).unwrap();
        assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF"));
        assert!(!path.with_extension("part").exists());
    }
}
