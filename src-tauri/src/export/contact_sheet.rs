//! Planches contact (cahier des charges, 4.6) : un PDF d'un album ou d'une
//! collection, pour un client ou pour l'archive. Grille de 2 à 8 colonnes,
//! en-tête sur chaque page (titre, précisions, logo du studio), et sous
//! chaque vignette son numéro et le nom du fichier. Vignettes tirées des
//! aperçus, retouches comprises ; filigrane texte ou logo en option.

use image::DynamicImage;
use image::imageops::FilterType;
use serde::Deserialize;

use crate::pdf::{Layout, Orientation, PageSize, Paper, gray};

const MARGIN: f32 = 36.0;
const GAP: f32 = 10.0;
const HEADER_H: f32 = 52.0;
const LOGO_W: f32 = 110.0;
const LOGO_H: f32 = 34.0;
/// Côté long des vignettes intégrées : net à l'impression, fichier léger.
const THUMB_SIDE: u32 = 600;
const WATERMARK_OPACITY: f32 = 0.35;

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "text")]
pub enum SheetWatermark {
    #[default]
    None,
    Text(String),
    /// Logo du studio (Réglages > Identité).
    Logo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetItem {
    pub media_id: i64,
    /// Nom du fichier (ou de la copie virtuelle).
    pub caption: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactSheet {
    pub title: String,
    /// Précisions sous le titre : dates, client et séance, photographe.
    pub subtitle: String,
    pub columns: u8,
    #[serde(default)]
    pub paper: Paper,
    #[serde(default)]
    pub orientation: Orientation,
    #[serde(default)]
    pub watermark: SheetWatermark,
    /// Logo du studio dans l'en-tête.
    #[serde(default)]
    pub header_logo: bool,
    /// Pied de page, `{page}` et `{pages}` remplacés.
    pub footer: String,
    pub items: Vec<SheetItem>,
}

/// Colonnes acceptées : de 2 à 8.
pub fn columns(n: u8) -> usize {
    usize::from(n.clamp(2, 8))
}

fn header(l: &mut Layout, sheet: &ContactSheet, logo: Option<&DynamicImage>) {
    let top = l.y;
    let logo = logo.filter(|_| sheet.header_logo);
    if let Some(logo) = logo {
        let right = l.page.w - l.margin;
        let _ = l.image(logo, right - LOGO_W, top - 4.0, LOGO_W, LOGO_H, false);
    }
    let room = l.inner_width() - if logo.is_some() { LOGO_W + 12.0 } else { 0.0 };
    let title = l.fit(&sheet.title, 14.0, room);
    l.text(l.margin, top, 14.0, gray(0.05), true, &title);
    let sub = l.fit(&sheet.subtitle, 8.5, room);
    l.text(l.margin, top + 20.0, 8.5, gray(0.4), false, &sub);
    l.y = top + HEADER_H - 10.0;
    let y = l.y;
    l.rule(y, 0.75);
    l.y += 10.0;
}

/// Le PDF de la planche. `thumb` donne l'image d'une photo (aperçu retouché) ;
/// `progress(fait, total)` suit les vignettes.
pub fn render(
    sheet: &ContactSheet,
    logo: Option<&DynamicImage>,
    thumb: impl Fn(i64) -> Option<DynamicImage>,
    mut progress: impl FnMut(usize, usize),
) -> Option<Vec<u8>> {
    let page = PageSize::new(sheet.paper, sheet.orientation);
    let mut l = Layout::new(&sheet.title, page, MARGIN)?;
    let cols = columns(sheet.columns);
    let cell_w = (l.inner_width() - GAP * (cols - 1) as f32) / cols as f32;
    // Cadre un peu plus haut que large : paysages et portraits y tiennent.
    let box_h = cell_w * 0.82;
    let caption = if cols >= 6 { 6.5 } else { 8.0 };
    let row_h = box_h + caption * 2.2 + GAP;

    l.new_page();
    header(&mut l, sheet, logo);
    let total = sheet.items.len();
    for (i, item) in sheet.items.iter().enumerate() {
        let col = i % cols;
        if col == 0 && i > 0 {
            l.y += row_h;
        }
        if col == 0 && l.y + row_h > l.bottom() {
            l.new_page();
            header(&mut l, sheet, logo);
        }
        let x = l.margin + col as f32 * (cell_w + GAP);
        let top = l.y;
        if let Some(img) = thumb(item.media_id) {
            let small = if img.width().max(img.height()) > THUMB_SIDE {
                img.resize(THUMB_SIDE, THUMB_SIDE, FilterType::Triangle)
            } else {
                img
            };
            if let Some((ix, iy, iw, ih)) = l.image(&small, x, top, cell_w, box_h, true) {
                match &sheet.watermark {
                    SheetWatermark::Text(text) if !text.trim().is_empty() => {
                        let size = (iw / 10.0).clamp(6.0, 18.0);
                        let t = l.fit(text.trim(), size, iw * 0.9);
                        let tw = l.width(&t, size);
                        l.with_opacity(WATERMARK_OPACITY, |l| {
                            l.text(
                                ix + (iw - tw) / 2.0,
                                iy + (ih - size) / 2.0,
                                size,
                                gray(1.0),
                                true,
                                &t,
                            );
                        });
                    }
                    SheetWatermark::Logo => {
                        if let Some(logo) = logo {
                            l.with_opacity(WATERMARK_OPACITY, |l| {
                                // Au milieu de la photo, sur 40 % de sa hauteur au plus.
                                let _ = l.image(logo, ix, iy + ih * 0.3, iw, ih * 0.4, true);
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        // Numéro d'ordre, puis le nom du fichier.
        let number = (i + 1).to_string();
        let ny = top + box_h + 3.0;
        l.text(x, ny, caption, gray(0.05), true, &number);
        let nw = l.width(&number, caption) + 4.0;
        let name = l.fit(&item.caption, caption, cell_w - nw);
        l.text(x + nw, ny, caption, gray(0.4), false, &name);
        progress(i + 1, total);
    }
    l.footer(&sheet.footer);
    Some(l.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(n: usize, cols: u8, watermark: SheetWatermark) -> ContactSheet {
        ContactSheet {
            title: "Mariage Dupont".into(),
            subtitle: "12 sept. 2026 · Lewis Bedar".into(),
            columns: cols,
            paper: Paper::A4,
            orientation: Orientation::Portrait,
            watermark,
            header_logo: true,
            footer: "Page {page} sur {pages}".into(),
            items: (1..=n as i64)
                .map(|i| SheetItem {
                    media_id: i,
                    caption: format!("DSC_{i:04}.NEF"),
                })
                .collect(),
        }
    }

    fn pages(bytes: &[u8]) -> usize {
        printpdf::PdfDocument::parse(bytes, &Default::default(), &mut Vec::new())
            .unwrap()
            .pages
            .len()
    }

    #[test]
    fn grille_pagination_et_filigranes() {
        let photo = |_| {
            Some(DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                900,
                600,
                image::Rgb([120, 140, 160]),
            )))
        };
        let logo = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            80,
            30,
            image::Rgba([0, 0, 0, 180]),
        ));
        // 4 colonnes en A4 portrait : 5 rangées par page, donc 20 photos.
        let mut seen = 0;
        let one = render(
            &sheet(20, 4, SheetWatermark::None),
            Some(&logo),
            photo,
            |d, _| seen = d,
        )
        .unwrap();
        assert_eq!((pages(&one), seen), (1, 20));
        let two = render(&sheet(21, 4, SheetWatermark::None), None, photo, |_, _| {}).unwrap();
        assert_eq!(pages(&two), 2);
        // 8 colonnes : 9 rangées, 72 photos par page.
        let eight = render(&sheet(72, 8, SheetWatermark::None), None, photo, |_, _| {}).unwrap();
        assert_eq!(pages(&eight), 1);
        // Les filigranes passent ; une photo sans aperçu garde sa place et son numéro.
        let text = render(
            &sheet(5, 3, SheetWatermark::Text("Épreuve".into())),
            Some(&logo),
            |id| (id != 3).then(|| photo(id)).flatten(),
            |_, _| {},
        )
        .unwrap();
        let logo_mark = render(
            &sheet(5, 3, SheetWatermark::Logo),
            Some(&logo),
            photo,
            |_, _| {},
        )
        .unwrap();
        assert!(text.starts_with(b"%PDF") && logo_mark.starts_with(b"%PDF"));
        // Colonnes hors bornes ramenées entre 2 et 8.
        assert_eq!((columns(0), columns(12)), (2, 8));
    }

    /// Planche d'exemple à regarder : `AFTERDUSK_SAMPLE_PDF=chemin cargo test -- --ignored planche_exemple`.
    #[test]
    #[ignore]
    fn planche_exemple() {
        let target = std::env::var("AFTERDUSK_SAMPLE_PDF").unwrap();
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-assets");
        let faces = [
            image::open(assets.join("visage_ouverts.jpg")).unwrap(),
            image::open(assets.join("visage_fermes.jpg")).unwrap(),
        ];
        let logo = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(240, 80, |x, y| {
            let on = (x / 20 + y / 20) % 2 == 0;
            image::Rgba([40, 60, 120, if on { 230 } else { 60 }])
        }));
        let mark = match std::env::var("AFTERDUSK_SAMPLE_MARK").as_deref() {
            Ok("logo") => SheetWatermark::Logo,
            _ => SheetWatermark::Text("© Lewis Bedar".into()),
        };
        let mut s = sheet(26, 4, mark);
        s.items[3].caption = "DSC_0004_tres_long_nom_de_fichier_export_final.NEF".into();
        let bytes = render(
            &s,
            Some(&logo),
            |id| Some(faces[(id % 2) as usize].clone()),
            |_, _| {},
        )
        .unwrap();
        std::fs::write(target, bytes).unwrap();
    }
}
