//! Éditeur de livraison simple (cahier des charges, V0.5) : une page au
//! format d'un modèle, de une à quatre photos, avec fond, marge, cadre,
//! textes et logo ; en grille, en bande photomaton ou en Polaroid. Pas de
//! calques libres ni de dessin. Le document est un JSON versionné ; la page
//! se rend ici à pleine résolution, l'interface n'en montre qu'un aperçu.
//!
//! Versions : 1, grille seule et textes en Inter ; 2, disposition et police
//! de chaque texte. Un document en version 1 se relit comme une grille en
//! Inter (`migrate`).

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::ExportError;
use super::render::Rendered;
use super::watermark;

/// Version du format des documents.
pub const VERSION: u32 = 2;
/// Une composition tient de une à quatre photos.
pub const MAX_PHOTOS: usize = 4;

/// Disposition des photos sur la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    /// Une à quatre photos en grille.
    #[default]
    Grid,
    /// Bande photomaton : les photos l'une sous l'autre, un bandeau libre en
    /// bas ; une page deux fois plus large porte deux bandes identiques.
    Strip,
    /// Polaroid : une photo carrée, bord large en bas pour la légende.
    Polaroid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Design {
    pub version: u32,
    /// Modèle de départ (format et usage).
    pub template: String,
    /// Absente en version 1 : grille.
    #[serde(default)]
    pub layout: Layout,
    /// Taille de la page, en pixels (le modèle, tourné au besoin).
    pub width: u32,
    pub height: u32,
    pub photos: Vec<DesignPhoto>,
    /// Fond et marge : couleur `#rrggbb`.
    pub background: String,
    /// Marge autour et entre les photos, en % du petit côté (0 à 20).
    pub margin: f64,
    pub frame: Option<Frame>,
    pub texts: Vec<DesignText>,
    pub logo: Option<DesignLogo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignPhoto {
    /// Identifiant de grille (négatif pour une copie virtuelle).
    pub item: i64,
    /// Point de la photo gardé au centre de sa case (fractions).
    pub focus: [f64; 2],
}

/// Filet autour de chaque photo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// Épaisseur, en % du petit côté (0 à 10).
    pub width: f64,
    pub color: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignText {
    pub text: String,
    /// Point d'accroche (bord gauche, milieu ou bord droit selon
    /// l'alignement ; haut de la première ligne), en fractions de la page.
    pub x: f64,
    pub y: f64,
    /// Corps, en % de la hauteur de la page.
    pub size: f64,
    /// Famille de police (livrée ou installée) ; absente en version 1 : Inter.
    #[serde(default = "default_font")]
    pub font: String,
    /// Graisse, de 100 à 900.
    pub weight: u16,
    pub color: String,
    pub align: Align,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignLogo {
    /// Coin haut gauche, en fractions de la page.
    pub x: f64,
    pub y: f64,
    /// Largeur, en % de la largeur de la page.
    pub width: f64,
    /// Opacité, de 0 à 100 %.
    pub opacity: f64,
}

fn default_font() -> String {
    crate::fonts::DEFAULT.to_owned()
}

/// Bandeau libre en bas d'une bande photomaton, en fraction de la hauteur.
pub const STRIP_BAND: f64 = 0.12;

/// Photo d'un Polaroid 600 (cadre de 88 × 107 mm, image de 79 × 79 mm) :
/// bords de 4,5 mm sur les côtés, 6 mm en haut, 22 mm en bas.
pub const POLAROID_CELL: [f64; 4] = [4.5 / 88.0, 6.0 / 107.0, 83.5 / 88.0, 85.0 / 107.0];

/// Document relu d'une version antérieure, mis à la version courante (les
/// champs nouveaux ont déjà pris leur valeur par défaut à la lecture).
pub fn migrate(mut design: Design) -> Design {
    if design.version < VERSION {
        design.version = VERSION;
    }
    design
}

impl Design {
    pub fn check(&self) -> Result<(), ExportError> {
        if self.version == 0 || self.version > VERSION {
            return Err(ExportError::Settings(format!(
                "composition en version {}, non lue par cette version d'Afterdusk",
                self.version
            )));
        }
        if self.photos.is_empty() || self.photos.len() > MAX_PHOTOS {
            return Err(ExportError::Settings(format!(
                "{} photos : une composition en tient de 1 à {MAX_PHOTOS}",
                self.photos.len()
            )));
        }
        if self.layout == Layout::Polaroid && self.photos.len() != 1 {
            return Err(ExportError::Settings(
                "un Polaroid ne tient qu'une photo".into(),
            ));
        }
        if self.width == 0 || self.height == 0 || self.width > 12_000 || self.height > 12_000 {
            return Err(ExportError::TooLarge);
        }
        Ok(())
    }
}

/// Couleur `#rrggbb` en sRGB codé (0 à 1) ; noir si illisible.
pub fn parse_color(hex: &str) -> [f32; 3] {
    let h = hex.trim().trim_start_matches('#');
    let channel = |i: usize| {
        h.get(i..i + 2)
            .and_then(|c| u8::from_str_radix(c, 16).ok())
            .map_or(0.0, |v| f32::from(v) / 255.0)
    };
    if h.len() == 6 {
        [channel(0), channel(2), channel(4)]
    } else {
        [0.0; 3]
    }
}

/// Cases des photos (gauche, haut, droite, bas, en fractions de la page)
/// pour `n` photos, une disposition, une page de rapport `aspect` (largeur
/// sur hauteur) et une marge en % du petit côté. La case `k` reçoit la
/// photo `k % n` (une bande doublée répète ses photos). Même calcul que
/// `ui/design.ts`.
pub fn cells(layout: Layout, n: usize, aspect: f64, margin: f64) -> Vec<[f64; 4]> {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    let m = margin.clamp(0.0, 20.0) / 100.0;
    // Marge en fractions de la largeur et de la hauteur.
    let (mx, my) = if aspect >= 1.0 {
        (m / aspect, m)
    } else {
        (m, m * aspect)
    };
    match layout {
        Layout::Grid => grid(n, aspect, mx, my),
        Layout::Strip => strip(n, aspect, mx, my),
        Layout::Polaroid if n > 0 => vec![POLAROID_CELL],
        Layout::Polaroid => Vec::new(),
    }
}

/// Bandes d'une page : une par tiers de hauteur de largeur pour une bande
/// photomaton (5 × 15 : une ; 10 × 15 : deux), une seule sinon. Textes et
/// logo se répètent sur chaque bande, décalés d'une largeur de bande.
pub fn columns(layout: Layout, aspect: f64) -> usize {
    match layout {
        Layout::Strip if aspect.is_finite() => ((aspect * 3.0).round() as usize).max(1),
        _ => 1,
    }
}

/// Bande : les photos l'une sous l'autre au-dessus du bandeau, dans
/// chaque colonne.
fn strip(n: usize, aspect: f64, mx: f64, my: f64) -> Vec<[f64; 4]> {
    if n == 0 {
        return Vec::new();
    }
    let columns = columns(Layout::Strip, aspect);
    let width = (1.0 - mx * (columns as f64 + 1.0)) / columns as f64;
    let bottom = 1.0 - STRIP_BAND;
    let height = (bottom - my * n as f64) / n as f64;
    let mut out = Vec::with_capacity(columns * n);
    for c in 0..columns {
        let left = mx + c as f64 * (width + mx);
        for i in 0..n {
            let top = my + i as f64 * (height + my);
            out.push([left, top, left + width, top + height]);
        }
    }
    out
}

fn grid(n: usize, aspect: f64, mx: f64, my: f64) -> Vec<[f64; 4]> {
    let (l, t, r, b) = (mx, my, 1.0 - mx, 1.0 - my);
    let mid_x = (l + r) / 2.0;
    let mid_y = (t + b) / 2.0;
    let (gx, gy) = (mx / 2.0, my / 2.0);
    let wide = aspect >= 1.0;
    match n {
        0 => Vec::new(),
        1 => vec![[l, t, r, b]],
        2 if wide => vec![[l, t, mid_x - gx, b], [mid_x + gx, t, r, b]],
        2 => vec![[l, t, r, mid_y - gy], [l, mid_y + gy, r, b]],
        3 if wide => vec![
            [l, t, mid_x - gx, b],
            [mid_x + gx, t, r, mid_y - gy],
            [mid_x + gx, mid_y + gy, r, b],
        ],
        3 => vec![
            [l, t, r, mid_y - gy],
            [l, mid_y + gy, mid_x - gx, b],
            [mid_x + gx, mid_y + gy, r, b],
        ],
        _ => vec![
            [l, t, mid_x - gx, mid_y - gy],
            [mid_x + gx, t, r, mid_y - gy],
            [l, mid_y + gy, mid_x - gx, b],
            [mid_x + gx, mid_y + gy, r, b],
        ],
    }
}

/// Case en pixels : x, y, largeur, hauteur.
fn pixels(cell: [f64; 4], w: u32, h: u32) -> (u32, u32, u32, u32) {
    let (wf, hf) = (f64::from(w), f64::from(h));
    let x0 = (cell[0] * wf).round().clamp(0.0, wf) as u32;
    let y0 = (cell[1] * hf).round().clamp(0.0, hf) as u32;
    let x1 = (cell[2] * wf).round().clamp(0.0, wf) as u32;
    let y1 = (cell[3] * hf).round().clamp(0.0, hf) as u32;
    (
        x0,
        y0,
        x1.saturating_sub(x0).max(1),
        y1.saturating_sub(y0).max(1),
    )
}

fn fill(img: &mut Rendered, x: u32, y: u32, w: u32, h: u32, color: [f32; 3]) {
    for yy in y..(y + h).min(img.height) {
        for xx in x..(x + w).min(img.width) {
            let d = ((yy * img.width + xx) * 3) as usize;
            img.rgb[d..d + 3].copy_from_slice(&color);
        }
    }
}

fn paste(page: &mut Rendered, part: &Rendered, x: u32, y: u32) {
    for row in 0..part.height.min(page.height.saturating_sub(y)) {
        let w = part.width.min(page.width.saturating_sub(x)) as usize * 3;
        let src = (row * part.width * 3) as usize;
        let dst = (((y + row) * page.width + x) * 3) as usize;
        page.rgb[dst..dst + w].copy_from_slice(&part.rgb[src..src + w]);
    }
}

/// Rend la page : `photo` fournit chaque photo déjà découpée à la taille de
/// sa case (largeur, hauteur). Valeurs sRGB codées, comme le rendu d'export.
pub fn compose(
    design: &Design,
    mut photo: impl FnMut(&DesignPhoto, u32, u32) -> Result<Rendered, ExportError>,
) -> Result<Rendered, ExportError> {
    design.check()?;
    let (w, h) = (design.width, design.height);
    let mut page = Rendered {
        width: w,
        height: h,
        rgb: parse_color(&design.background).repeat((w as usize) * (h as usize)),
    };
    let short = f64::from(w.min(h));
    let aspect = f64::from(w) / f64::from(h);
    let n = design.photos.len();
    for (k, cell) in cells(design.layout, n, aspect, design.margin)
        .into_iter()
        .enumerate()
    {
        let p = &design.photos[k % n];
        let (x, y, cw, ch) = pixels(cell, w, h);
        let part = photo(p, cw, ch)?;
        paste(&mut page, &part, x, y);
        if let Some(frame) = &design.frame {
            let t = (short * frame.width.clamp(0.0, 10.0) / 100.0).round() as u32;
            if t > 0 {
                let c = parse_color(&frame.color);
                let t = t.min(cw / 2).min(ch / 2);
                fill(&mut page, x, y, cw, t, c);
                fill(&mut page, x, y + ch - t, cw, t, c);
                fill(&mut page, x, y, t, ch, c);
                fill(&mut page, x + cw - t, y, t, ch, c);
            }
        }
    }
    let repeat = columns(design.layout, aspect);
    // Décalage d'une bande à la suivante, en pixels.
    let shift = |c: usize| (f64::from(w) * c as f64 / repeat as f64).round() as i64;
    if let Some(logo) = &design.logo
        && let Some(path) = watermark::studio_logo_path()
    {
        let target = (f64::from(w) * logo.width.clamp(1.0, 100.0) / 100.0).round() as u32;
        if let Some(stamp) = watermark::image_stamp(&path, target.max(1))? {
            let opacity = (logo.opacity.clamp(0.0, 100.0) / 100.0) as f32;
            let (x0, y0) = (
                (logo.x * f64::from(w)).round() as i64,
                (logo.y * f64::from(h)).round() as i64,
            );
            for c in 0..repeat {
                watermark::blend(&mut page, &stamp, x0 + shift(c), y0, opacity);
            }
        }
    }
    for text in &design.texts {
        let px = (f64::from(h) * text.size.clamp(0.5, 50.0) / 100.0) as f32;
        let ink = parse_color(&text.color);
        // Hauteur de ligne : 1,2 fois le corps, le texte centré sur la ligne,
        // comme Konva dans l'éditeur.
        let line_height = f64::from(px) * 1.2;
        let mut middle = text.y * f64::from(h) + line_height / 2.0;
        for line in text.text.lines() {
            if let Some(stamp) = watermark::text_line(line, px, &text.font, text.weight, ink)? {
                let anchor = (text.x * f64::from(w)).round() as i64;
                let left = match text.align {
                    Align::Left => anchor,
                    Align::Center => anchor - i64::from(stamp.width) / 2,
                    Align::Right => anchor - i64::from(stamp.width),
                };
                let top = (middle - f64::from(stamp.height) / 2.0).round() as i64;
                for c in 0..repeat {
                    watermark::blend(&mut page, &stamp, left + shift(c), top, 1.0);
                }
            }
            middle += line_height;
        }
    }
    Ok(page)
}

/// Une composition enregistrée.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub id: i64,
    pub name: String,
    pub document: Design,
    pub updated_at: String,
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, String, String, String)> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
}

fn decode((id, name, json, updated_at): (i64, String, String, String)) -> Option<Saved> {
    // Un document illisible (version future) est ignoré plutôt que bloquant.
    let document = migrate(serde_json::from_str::<Design>(&json).ok()?);
    document.check().ok()?;
    Some(Saved {
        id,
        name,
        document,
        updated_at,
    })
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Saved>> {
    let rows: Vec<_> = conn
        .prepare(
            "SELECT id, name, document, updated_at FROM delivery_designs
              ORDER BY updated_at DESC, id DESC",
        )?
        .query_map([], row)?
        .collect::<Result<_, _>>()?;
    Ok(rows.into_iter().filter_map(decode).collect())
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Saved>> {
    Ok(conn
        .query_row(
            "SELECT id, name, document, updated_at FROM delivery_designs WHERE id = ?1",
            [id],
            row,
        )
        .optional()?
        .and_then(decode))
}

/// Enregistre (nouvelle composition si `id` est absent) ; renvoie l'identifiant.
pub fn save(
    conn: &Connection,
    id: Option<i64>,
    name: &str,
    document: &Design,
) -> Result<i64, ExportError> {
    document.check()?;
    let json = serde_json::to_string(document).map_err(|e| ExportError::Settings(e.to_string()))?;
    let now = crate::db::now_utc();
    let sql = |e: rusqlite::Error| ExportError::Settings(e.to_string());
    match id {
        Some(id) => {
            conn.execute(
                "UPDATE delivery_designs SET name = ?2, document = ?3, updated_at = ?4 WHERE id = ?1",
                params![id, name, json, now],
            )
            .map_err(sql)?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO delivery_designs (name, document, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)",
                params![name, json, now],
            )
            .map_err(sql)?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// Une composition ne contient que sa mise en page : la supprimer ne touche
/// à aucune photo.
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM delivery_designs WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design(n: usize) -> Design {
        Design {
            version: VERSION,
            template: "square".into(),
            layout: Layout::Grid,
            width: 200,
            height: 100,
            photos: (0..n)
                .map(|i| DesignPhoto {
                    item: i as i64 + 1,
                    focus: [0.5, 0.5],
                })
                .collect(),
            background: "#ffffff".into(),
            margin: 10.0,
            frame: None,
            texts: Vec::new(),
            logo: None,
        }
    }

    fn at(img: &Rendered, x: u32, y: u32) -> [f32; 3] {
        let d = ((y * img.width + x) * 3) as usize;
        [img.rgb[d], img.rgb[d + 1], img.rgb[d + 2]]
    }

    #[test]
    fn couleurs() {
        assert_eq!(parse_color("#ff8000"), [1.0, 128.0 / 255.0, 0.0]);
        assert_eq!(parse_color("zz"), [0.0; 3]);
    }

    #[test]
    fn cases_selon_le_nombre_et_l_orientation() {
        // Page 2:1, marge de 10 % du petit côté : 5 % de la largeur, 10 % de la hauteur.
        let c = cells(Layout::Grid, 2, 2.0, 10.0);
        assert_eq!(c.len(), 2);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(c[0][0], 0.05) && close(c[0][1], 0.1) && close(c[0][2], 0.475));
        assert!(close(c[1][0], 0.525) && close(c[1][2], 0.95) && close(c[1][3], 0.9));
        // Page en hauteur : l'une sous l'autre.
        let c = cells(Layout::Grid, 2, 0.5, 10.0);
        assert!(close(c[0][2], 0.9) && close(c[0][3], 0.475) && close(c[1][1], 0.525));
        assert_eq!(cells(Layout::Grid, 3, 1.5, 0.0).len(), 3);
        let four = cells(Layout::Grid, 4, 1.0, 0.0);
        assert_eq!(four[3], [0.5, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn page_fond_photos_et_cadre() {
        let mut d = design(2);
        d.frame = Some(Frame {
            width: 2.0,
            color: "#ff0000".into(),
        });
        let mut sizes = Vec::new();
        let page = compose(&d, |_, w, h| {
            sizes.push((w, h));
            Ok(Rendered {
                width: w,
                height: h,
                rgb: vec![0.0; (w * h * 3) as usize],
            })
        })
        .unwrap();
        assert_eq!(sizes, vec![(85, 80), (85, 80)]);
        // Fond dans la marge, filet rouge au bord de la case, photo au milieu.
        assert_eq!(at(&page, 2, 2), [1.0; 3]);
        assert_eq!(at(&page, 10, 10), [1.0, 0.0, 0.0]);
        assert_eq!(at(&page, 50, 50), [0.0; 3]);
    }

    #[test]
    fn texte_aligne() {
        let mut d = design(1);
        d.margin = 0.0;
        d.texts.push(DesignText {
            text: "Mariage".into(),
            x: 0.5,
            y: 0.4,
            size: 20.0,
            font: "Inter".into(),
            weight: 700,
            color: "#000000".into(),
            align: Align::Center,
        });
        let page = compose(&d, |_, w, h| {
            Ok(Rendered {
                width: w,
                height: h,
                rgb: vec![1.0; (w * h * 3) as usize],
            })
        })
        .unwrap();
        // De l'encre de part et d'autre du milieu, rien dans les coins.
        let ink = |x0: u32, x1: u32| (x0..x1).any(|x| (40..65).any(|y| at(&page, x, y)[0] < 0.5));
        assert!(ink(60, 100) && ink(100, 140));
        assert!(!ink(0, 30) && !ink(170, 200));
    }

    #[test]
    fn bande_et_polaroid() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        // Bande 1:3, quatre photos, sans marge : quatre cases de 0,22 de haut.
        let c = cells(Layout::Strip, 4, 1.0 / 3.0, 0.0);
        assert_eq!(c.len(), 4);
        assert!(close(c[3][3], 1.0 - STRIP_BAND) && close(c[0][2], 1.0));
        // Bande doublée (2:3) : deux colonnes qui répètent les photos.
        let d = cells(Layout::Strip, 3, 2.0 / 3.0, 3.0);
        assert_eq!(d.len(), 6);
        assert!(close(d[0][1], d[3][1]) && d[3][0] > 0.5);
        assert_eq!(cells(Layout::Polaroid, 1, 0.82, 10.0), vec![POLAROID_CELL]);
        let mut p = design(2);
        p.layout = Layout::Polaroid;
        assert!(p.check().is_err());
    }

    #[test]
    fn corps_au_sens_de_css_et_texte_repete_sur_chaque_bande() {
        // Inter : cadratin de 2048 unités, hauteur totale de 2472 environ ;
        // un corps de 100 px donne une ligne d'environ 121 px.
        let stamp = watermark::text_line("Hg", 100.0, "Inter", 400, [0.0; 3])
            .unwrap()
            .unwrap();
        assert!((115..=128).contains(&stamp.height), "{}", stamp.height);
        // Bande doublée : le texte posé dans la première bande se retrouve
        // dans la seconde.
        let mut d = design(1);
        d.layout = Layout::Strip;
        d.width = 200;
        d.height = 300;
        d.margin = 0.0;
        d.texts.push(DesignText {
            text: "A".into(),
            x: 0.25,
            y: 0.9,
            size: 5.0,
            font: "Inter".into(),
            weight: 700,
            color: "#000000".into(),
            align: Align::Center,
        });
        let page = compose(&d, |_, w, h| {
            Ok(Rendered {
                width: w,
                height: h,
                rgb: vec![1.0; (w * h * 3) as usize],
            })
        })
        .unwrap();
        let ink = |x0: u32, x1: u32| (x0..x1).any(|x| (265..295).any(|y| at(&page, x, y)[0] < 0.5));
        assert!(ink(40, 60) && ink(140, 160));
        assert!(!ink(0, 30) && !ink(170, 200));
    }

    #[test]
    fn document_en_version_1_relu() {
        let json = r##"{"version":1,"template":"square","width":100,"height":100,
            "photos":[{"item":3,"focus":[0.5,0.5]}],"background":"#ffffff","margin":0,
            "frame":null,"texts":[{"text":"A","x":0,"y":0,"size":5,"weight":400,
            "color":"#000000","align":"left"}],"logo":null}"##;
        let d = migrate(serde_json::from_str::<Design>(json).unwrap());
        assert_eq!(d.version, VERSION);
        assert_eq!(d.layout, Layout::Grid);
        assert_eq!(d.texts[0].font, "Inter");
    }

    #[test]
    fn document_verifie() {
        assert!(design(0).check().is_err());
        assert!(design(5).check().is_err());
        let mut d = design(1);
        d.version = VERSION + 1;
        assert!(d.check().is_err());
        assert!(design(4).check().is_ok());
    }

    #[test]
    fn enregistrement() {
        let conn = crate::db::open_in_memory().unwrap();
        let mut d = design(1);
        let id = save(&conn, None, "Faire-part", &d).unwrap();
        d.background = "#000000".into();
        save(&conn, Some(id), "Faire-part noir", &d).unwrap();
        let s = get(&conn, id).unwrap().unwrap();
        assert_eq!(s.name, "Faire-part noir");
        assert_eq!(s.document.background, "#000000");
        assert_eq!(list(&conn).unwrap().len(), 1);
        delete(&conn, id).unwrap();
        assert!(get(&conn, id).unwrap().is_none());
    }
}
