//! Mise en page PDF commune (printpdf, décision de l'export d'inventaire) :
//! inventaire du matériel, planches contact. Texte en Inter, mesuré sur les
//! largeurs des glyphes ; images, filets, tableaux ; saut de page. Le fichier
//! est écrit à côté puis renommé : un export interrompu ne laisse pas de
//! fichier à moitié écrit.

use std::path::Path;

use image::{DynamicImage, RgbaImage};
use printpdf::{
    Color, ExtendedGraphicsState, Line, LinePoint, Op, ParsedFont, PdfDocument, PdfFontHandle,
    PdfPage, PdfSaveOptions, Point, Pt, RawImage, RawImageData, RawImageFormat, Rgb, TextItem,
    TextRenderingMode, XObjectTransform,
};
use serde::Deserialize;

/// Inter, déjà embarquée pour l'interface (licence OFL).
const FONT: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Paper {
    #[default]
    A4,
    Letter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

/// Dimensions de page en points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    pub w: f32,
    pub h: f32,
}

impl PageSize {
    pub fn new(paper: Paper, orientation: Orientation) -> Self {
        let (w, h) = match paper {
            Paper::A4 => (595.28, 841.89),
            Paper::Letter => (612.0, 792.0),
        };
        match orientation {
            Orientation::Portrait => Self { w, h },
            Orientation::Landscape => Self { w: h, h: w },
        }
    }
}

pub fn gray(v: f32) -> Color {
    Color::Rgb(Rgb::new(v, v, v, None))
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let part = path.with_extension("part");
    std::fs::write(&part, bytes)
        .and_then(|()| std::fs::rename(&part, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&part);
        })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Largeurs relatives des colonnes ; vides, toutes égales.
    #[serde(default)]
    pub widths: Vec<f32>,
}

/// Mise en page par lignes, du haut vers le bas, avec saut de page.
pub struct Layout {
    doc: PdfDocument,
    font: ParsedFont,
    font_handle: PdfFontHandle,
    pages: Vec<Vec<Op>>,
    /// Page où s'écrivent les opérations.
    current: usize,
    /// Distance depuis le haut de la page courante.
    pub y: f32,
    pub page: PageSize,
    pub margin: f32,
}

impl Layout {
    pub fn new(title: &str, page: PageSize, margin: f32) -> Option<Self> {
        let font = ParsedFont::from_bytes(FONT, 0, &mut Vec::new())?;
        let mut doc = PdfDocument::new(title);
        let font_id = doc.add_font(&font);
        Some(Self {
            doc,
            font,
            font_handle: PdfFontHandle::External(font_id),
            pages: Vec::new(),
            current: 0,
            y: margin,
            page,
            margin,
        })
    }

    /// Largeur utile, entre les marges.
    pub fn inner_width(&self) -> f32 {
        self.page.w - 2.0 * self.margin
    }

    /// Bas de la zone utile (le pied de page est en dessous).
    pub fn bottom(&self) -> f32 {
        self.page.h - self.margin - 14.0
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn ops(&mut self) -> &mut Vec<Op> {
        if self.pages.is_empty() {
            self.new_page();
        }
        &mut self.pages[self.current]
    }

    pub fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.current = self.pages.len() - 1;
        self.y = self.margin;
    }

    /// Place la suite sur une nouvelle page si `height` ne tient plus.
    pub fn ensure(&mut self, height: f32) -> bool {
        if self.pages.is_empty() || self.y + height > self.bottom() {
            self.new_page();
            return true;
        }
        false
    }

    pub fn width(&self, text: &str, size: f32) -> f32 {
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
    pub fn fit(&self, text: &str, size: f32, max: f32) -> String {
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
    pub fn text(&mut self, x: f32, top: f32, size: f32, color: Color, bold: bool, text: &str) {
        let font = self.font_handle.clone();
        let base = self.page.h;
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
                    y: Pt(base - top - size * 0.8),
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

    /// Filet sur toute la largeur utile.
    pub fn rule(&mut self, top: f32, shade: f32) {
        let y = self.page.h - top;
        let (left, right) = (self.margin, self.page.w - self.margin);
        let point = |x| LinePoint {
            p: Point { x: Pt(x), y: Pt(y) },
            bezier: false,
        };
        self.ops().extend([
            Op::SetOutlineColor { col: gray(shade) },
            Op::SetOutlineThickness { pt: Pt(0.5) },
            Op::DrawLine {
                line: Line {
                    points: vec![point(left), point(right)],
                    is_closed: false,
                },
            },
        ]);
    }

    pub fn table(&mut self, table: &Table, size: f32) {
        let inner = self.inner_width();
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
            let mut x = l.margin;
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
            let mut x = self.margin;
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

    /// Image dans un cadre (`left`, `top`, `w` × `h`), proportions gardées,
    /// centrée dans le cadre si `center`. Renvoie le rectangle dessiné
    /// (gauche, haut, largeur, hauteur).
    pub fn image(
        &mut self,
        img: &DynamicImage,
        left: f32,
        top: f32,
        w: f32,
        h: f32,
        center: bool,
    ) -> Option<(f32, f32, f32, f32)> {
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        if iw <= 0.0 || ih <= 0.0 {
            return None;
        }
        let scale = (w / iw).min(h / ih);
        let (dw, dh) = (iw * scale, ih * scale);
        let (x, y) = if center {
            (left + (w - dw) / 2.0, top + (h - dh) / 2.0)
        } else {
            (left, top)
        };
        let raw = if img.color().has_alpha() {
            let rgba: RgbaImage = img.to_rgba8();
            RawImage {
                pixels: RawImageData::U8(rgba.into_raw()),
                width: iw as usize,
                height: ih as usize,
                data_format: RawImageFormat::RGBA8,
                tag: Vec::new(),
            }
        } else {
            RawImage {
                pixels: RawImageData::U8(img.to_rgb8().into_raw()),
                width: iw as usize,
                height: ih as usize,
                data_format: RawImageFormat::RGB8,
                tag: Vec::new(),
            }
        };
        let id = self.doc.add_image(&raw);
        let base = self.page.h;
        self.ops().push(Op::UseXobject {
            id,
            transform: XObjectTransform {
                translate_x: Some(Pt(x)),
                translate_y: Some(Pt(base - y - dh)),
                // À 72 ppp, un pixel vaut un point ; l'échelle fait le reste.
                dpi: Some(72.0),
                scale_x: Some(scale),
                scale_y: Some(scale),
                ..Default::default()
            },
        });
        Some((x, y, dw, dh))
    }

    /// Dessine `draw` avec une opacité (filigrane) : texte et images.
    pub fn with_opacity(&mut self, alpha: f32, draw: impl FnOnce(&mut Layout)) {
        let gs = self.doc.add_graphics_state(
            ExtendedGraphicsState::default()
                .with_current_fill_alpha(alpha)
                .with_current_stroke_alpha(alpha),
        );
        self.ops()
            .extend([Op::SaveGraphicsState, Op::LoadGraphicsState { gs }]);
        draw(self);
        self.ops().push(Op::RestoreGraphicsState);
    }

    /// Numéro de page en bas à droite de chaque page ; `{page}` et `{pages}`
    /// remplacés.
    pub fn footer(&mut self, template: &str) {
        let total = self.pages.len();
        let size = 8.0;
        for i in 0..total {
            let text = template
                .replace("{page}", &(i + 1).to_string())
                .replace("{pages}", &total.to_string());
            let w = self.width(&text, size);
            self.current = i;
            let (x, top) = (
                self.page.w - self.margin - w,
                self.page.h - self.margin + 6.0,
            );
            self.text(x, top, size, gray(0.5), false, &text);
        }
    }

    /// Le document : images recompressées en JPEG par printpdf.
    pub fn finish(mut self) -> Vec<u8> {
        let (w, h) = (self.page.w, self.page.h);
        let pages = self
            .pages
            .into_iter()
            .map(|ops| PdfPage::new(Pt(w).into(), Pt(h).into(), ops))
            .collect();
        self.doc
            .with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_de_page() {
        let a4 = PageSize::new(Paper::A4, Orientation::Portrait);
        let letter = PageSize::new(Paper::Letter, Orientation::Landscape);
        assert!(a4.h > a4.w);
        assert_eq!((letter.w, letter.h), (792.0, 612.0));
    }

    #[test]
    fn texte_coupe_et_image_cadree() {
        let mut l = Layout::new(
            "Essai",
            PageSize::new(Paper::A4, Orientation::Portrait),
            40.0,
        )
        .unwrap();
        let cut = l.fit("Un titre beaucoup trop long pour sa colonne", 10.0, 60.0);
        assert!(cut.ends_with('…') && l.width(&cut, 10.0) <= 60.0, "{cut}");
        // Image large dans un cadre carré : centrée verticalement.
        let img = DynamicImage::ImageRgb8(image::RgbImage::new(200, 100));
        let (x, y, w, h) = l.image(&img, 40.0, 100.0, 100.0, 100.0, true).unwrap();
        assert_eq!((x, w, h), (40.0, 100.0, 50.0));
        assert_eq!(y, 125.0);
        let bytes = l.finish();
        assert!(bytes.starts_with(b"%PDF"));
    }
}
