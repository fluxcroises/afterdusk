//! Export de l'inventaire (cahier des charges, 4.8) : CSV pour un tableur,
//! PDF pour l'assurance ou l'archive. L'interface fournit les textes déjà
//! traduits et mis en forme ; le cœur écrit le fichier et va chercher la
//! photo de chaque appareil. Le fichier est écrit à côté puis renommé : un
//! export interrompu ne laisse pas de fichier à moitié écrit.

use std::path::Path;

use printpdf::{
    Color, Line, LinePoint, Op, ParsedFont, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions,
    Point, Pt, RawImage, Rgb, TextItem, TextRenderingMode, XObjectTransform,
};
use serde::Deserialize;

use super::GearError;

/// Inter, déjà embarquée pour l'interface (licence OFL).
const FONT: &[u8] = include_bytes!("../../assets/fonts/InterVariable.ttf");

/// A4 portrait, en points.
const PAGE_W: f32 = 595.28;
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 48.0;
const PHOTO_W: f32 = 150.0;
const PHOTO_H: f32 = 100.0;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Largeurs relatives des colonnes ; vides, toutes égales.
    #[serde(default)]
    pub widths: Vec<f32>,
}

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
    let part = path.with_extension("part");
    std::fs::write(&part, bytes)
        .and_then(|()| std::fs::rename(&part, path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&part);
            GearError::Export(e.to_string())
        })
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

fn gray(v: f32) -> Color {
    Color::Rgb(Rgb::new(v, v, v, None))
}

/// Mise en page par lignes, du haut vers le bas, avec saut de page.
struct Layout {
    font: ParsedFont,
    font_handle: PdfFontHandle,
    pages: Vec<Vec<Op>>,
    /// Page où s'écrivent les opérations.
    current: usize,
    /// Distance depuis le haut de la page courante.
    y: f32,
}

impl Layout {
    fn ops(&mut self) -> &mut Vec<Op> {
        if self.pages.is_empty() {
            self.new_page();
        }
        &mut self.pages[self.current]
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.current = self.pages.len() - 1;
        self.y = MARGIN;
    }

    /// Place la suite sur une nouvelle page si `height` ne tient plus.
    fn ensure(&mut self, height: f32) -> bool {
        if self.pages.is_empty() || self.y + height > PAGE_H - MARGIN - 14.0 {
            self.new_page();
            return true;
        }
        false
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        let units: u32 = text
            .chars()
            .map(|c| {
                self.font
                    .lookup_glyph_index(c as u32)
                    .and_then(|g| self.font.get_glyph_width(g))
                    .map_or(0, u32::from)
            })
            .sum();
        units as f32 * size / f32::from(self.font.units_per_em.max(1))
    }

    /// Coupe le texte à `max` points, avec des points de suspension.
    fn fit(&self, text: &str, size: f32, max: f32) -> String {
        if self.width(text, size) <= max {
            return text.to_owned();
        }
        let mut out: String = text.to_owned();
        while !out.is_empty() && self.width(&format!("{out}…"), size) > max {
            out.pop();
        }
        format!("{}…", out.trim_end())
    }

    /// Texte dont le haut est à `top` (depuis le haut de page).
    fn text(&mut self, x: f32, top: f32, size: f32, color: Color, bold: bool, text: &str) {
        let font = self.font_handle.clone();
        let mut ops = vec![
            Op::StartTextSection,
            Op::SetFont {
                font,
                size: Pt(size),
            },
            Op::SetFillColor { col: color.clone() },
        ];
        if bold {
            // Inter variable : le gras est simulé par un léger contour.
            ops.push(Op::SetTextRenderingMode {
                mode: TextRenderingMode::FillStroke,
            });
            ops.push(Op::SetOutlineColor { col: color });
            ops.push(Op::SetOutlineThickness {
                pt: Pt(size * 0.03),
            });
        }
        ops.extend([
            Op::SetTextCursor {
                pos: Point {
                    x: Pt(x),
                    y: Pt(PAGE_H - top - size * 0.8),
                },
            },
            Op::ShowText {
                items: vec![TextItem::Text(text.to_owned())],
            },
        ]);
        if bold {
            ops.push(Op::SetTextRenderingMode {
                mode: TextRenderingMode::Fill,
            });
        }
        ops.push(Op::EndTextSection);
        self.ops().extend(ops);
    }

    fn rule(&mut self, top: f32, shade: f32) {
        let y = PAGE_H - top;
        let point = |x| LinePoint {
            p: Point { x: Pt(x), y: Pt(y) },
            bezier: false,
        };
        self.ops().extend([
            Op::SetOutlineColor { col: gray(shade) },
            Op::SetOutlineThickness { pt: Pt(0.5) },
            Op::DrawLine {
                line: Line {
                    points: vec![point(MARGIN), point(PAGE_W - MARGIN)],
                    is_closed: false,
                },
            },
        ]);
    }

    fn table(&mut self, table: &Table, size: f32) {
        let inner = PAGE_W - 2.0 * MARGIN;
        let n = table.headers.len().max(1);
        let weights: Vec<f32> = if table.widths.len() == n {
            table.widths.clone()
        } else {
            vec![1.0; n]
        };
        let total: f32 = weights.iter().sum::<f32>().max(f32::EPSILON);
        let widths: Vec<f32> = weights.iter().map(|w| w / total * inner).collect();
        let row_h = size * 1.9;
        let header = |l: &mut Layout| {
            let mut x = MARGIN;
            let top = l.y;
            for (h, w) in table.headers.iter().zip(&widths) {
                let t = l.fit(h, size * 0.9, w - 6.0);
                l.text(x, top + 4.0, size * 0.9, gray(0.4), false, &t);
                x += w;
            }
            l.y += row_h;
            let y = l.y;
            l.rule(y, 0.55);
        };
        self.ensure(row_h * 2.0);
        header(self);
        for row in &table.rows {
            if self.ensure(row_h) {
                header(self);
            }
            let mut x = MARGIN;
            let top = self.y;
            for (cell, w) in row.iter().zip(&widths) {
                let t = self.fit(cell, size, w - 6.0);
                self.text(x, top + 5.0, size, gray(0.1), false, &t);
                x += w;
            }
            self.y += row_h;
            let y = self.y;
            self.rule(y, 0.88);
        }
    }

    fn image(&mut self, doc: &mut PdfDocument, bytes: &[u8], left: f32, top: f32) -> bool {
        let Ok(image) = RawImage::decode_from_bytes(bytes, &mut Vec::new()) else {
            return false;
        };
        let (w, h) = (image.width as f32, image.height as f32);
        if w <= 0.0 || h <= 0.0 {
            return false;
        }
        // Tient dans le cadre, proportions gardées.
        let scale = (PHOTO_W / w).min(PHOTO_H / h);
        let dh = h * scale;
        let id = doc.add_image(&image);
        self.ops().push(Op::UseXobject {
            id,
            transform: XObjectTransform {
                translate_x: Some(Pt(left)),
                translate_y: Some(Pt(PAGE_H - top - dh)),
                // À 72 ppp, un pixel vaut un point ; l'échelle fait le reste.
                dpi: Some(72.0),
                scale_x: Some(scale),
                scale_y: Some(scale),
                ..Default::default()
            },
        });
        true
    }
}

/// PDF de l'inventaire : tableau récapitulatif, puis une fiche par appareil.
/// `photo` donne l'image d'un appareil (JPEG ou PNG), s'il en a une.
pub fn pdf(
    doc_in: &InventoryPdf,
    photo: impl Fn(i64) -> Option<Vec<u8>>,
) -> Result<Vec<u8>, GearError> {
    let font = ParsedFont::from_bytes(FONT, 0, &mut Vec::new())
        .ok_or_else(|| GearError::Export("police illisible".into()))?;
    let mut doc = PdfDocument::new(&doc_in.title);
    let font_id = doc.add_font(&font);
    let mut l = Layout {
        font,
        font_handle: PdfFontHandle::External(font_id),
        pages: Vec::new(),
        current: 0,
        y: MARGIN,
    };

    l.ensure(60.0);
    l.text(MARGIN, l.y, 18.0, gray(0.05), true, &doc_in.title);
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
        let has_photo = photo(s.gear_id).is_some_and(|b| l.image(&mut doc, &b, MARGIN, block));
        let x = if has_photo {
            MARGIN + PHOTO_W + 18.0
        } else {
            MARGIN
        };
        let label_w = 110.0;
        let value_w = PAGE_W - MARGIN - x - label_w;
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

    let total = l.pages.len();
    let footer_size = 8.0;
    for i in 0..total {
        let text = doc_in
            .footer
            .replace("{page}", &(i + 1).to_string())
            .replace("{pages}", &total.to_string());
        let w = l.width(&text, footer_size);
        l.current = i;
        l.text(
            PAGE_W - MARGIN - w,
            PAGE_H - MARGIN + 6.0,
            footer_size,
            gray(0.5),
            false,
            &text,
        );
    }

    let pages = l
        .pages
        .into_iter()
        .map(|ops| PdfPage::new(Pt(PAGE_W).into(), Pt(PAGE_H).into(), ops))
        .collect();
    Ok(doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new()))
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
    photo: impl Fn(i64) -> Option<Vec<u8>>,
) -> Result<(), GearError> {
    write_atomic(path, &pdf(doc, photo)?)
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
        let bytes = pdf(&doc, |_| Some(jpeg.clone())).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        // Relu par printpdf : 160 lignes de tableau ne tiennent pas sur une page.
        let parsed = PdfDocument::parse(&bytes, &Default::default(), &mut Vec::new()).unwrap();
        assert!(parsed.pages.len() >= 4, "{} pages", parsed.pages.len());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inventaire.pdf");
        write_pdf(&path, &doc, |_| None).unwrap();
        assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF"));
        assert!(!path.with_extension("part").exists());
    }
}
