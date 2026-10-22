//! Modèles de livraison (cahier des charges, V0.5) : formats fixes pour les
//! réseaux, le tirage et le web. Chaque photo reçoit pour chaque modèle un
//! cadrage au bon rapport, centré par défaut sur le plus grand visage, que
//! l'utilisateur peut déplacer ou resserrer. Les cadrages sont des fractions
//! de l'image retouchée (gauche, haut, droite, bas).

use std::collections::HashMap;
use std::path::Path;

use image::RgbImage;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::render::Sharpen;
use crate::analysis::eyes;
use crate::raw::preview;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Social,
    Print,
    Web,
    /// Pages de l'éditeur de compositions (bande photomaton, Polaroid).
    Creative,
}

/// Un modèle : identifiant (clé de traduction), usage, taille en pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: &'static str,
    pub group: Group,
    pub width: u32,
    pub height: u32,
}

const fn t(id: &'static str, group: Group, width: u32, height: u32) -> Template {
    Template {
        id,
        group,
        width,
        height,
    }
}

/// Modèles intégrés, dans l'ordre de l'interface. Tirages à 300 dpi.
pub const TEMPLATES: &[Template] = &[
    t("instagram-portrait", Group::Social, 1080, 1350),
    t("square", Group::Social, 1080, 1080),
    t("story", Group::Social, 1080, 1920),
    t("facebook-post", Group::Social, 1200, 630),
    t("facebook-cover", Group::Social, 1640, 624),
    t("pinterest", Group::Social, 1000, 1500),
    t("youtube", Group::Social, 1280, 720),
    t("x-post", Group::Social, 1600, 900),
    t("print-10x15", Group::Print, 1800, 1200),
    t("print-13x18", Group::Print, 2126, 1535),
    t("print-20x30", Group::Print, 3543, 2362),
    t("print-a4", Group::Print, 3508, 2480),
    t("web-fullscreen", Group::Web, 2560, 1440),
    // Bande de 5 × 15 cm et Polaroid 600 (88 × 107 mm), à 300 dpi.
    t("photo-strip", Group::Creative, 600, 1800),
    t("polaroid", Group::Creative, 1040, 1264),
];

pub fn find(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.id == id)
}

impl Template {
    /// Taille de sortie pour une photo de `w` × `h` : un tirage suit
    /// l'orientation de la photo, un format de réseau reste tel quel.
    pub fn size_for(&self, w: u32, h: u32) -> (u32, u32) {
        let portrait = h > w;
        if self.group == Group::Print && portrait != (self.height > self.width) {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    pub fn sharpen(&self) -> Sharpen {
        match self.group {
            Group::Print | Group::Creative => Sharpen::Paper,
            Group::Social | Group::Web => Sharpen::Screen,
        }
    }
}

/// Plus grand cadre de rapport `ratio` (largeur sur hauteur) dans une image
/// de `w` × `h`, centré au plus près de `center` (fractions).
pub fn fit(ratio: f64, w: u32, h: u32, center: (f64, f64)) -> [f64; 4] {
    let (wf, hf) = (f64::from(w.max(1)), f64::from(h.max(1)));
    let (fw, fh) = if wf / hf > ratio {
        (hf * ratio / wf, 1.0)
    } else {
        (1.0, wf / ratio / hf)
    };
    place(fw, fh, center)
}

fn place(fw: f64, fh: f64, center: (f64, f64)) -> [f64; 4] {
    let left = (center.0 - fw / 2.0).clamp(0.0, 1.0 - fw);
    let top = (center.1 - fh / 2.0).clamp(0.0, 1.0 - fh);
    [left, top, left + fw, top + fh]
}

/// Cadrage enregistré ramené au rapport voulu et dans l'image : la photo a
/// pu être recadrée depuis. Largeur gardée, hauteur ajustée, centre gardé.
pub fn conform(rect: [f64; 4], ratio: f64, w: u32, h: u32) -> [f64; 4] {
    let (wf, hf) = (f64::from(w.max(1)), f64::from(h.max(1)));
    let center = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
    let mut fw = (rect[2] - rect[0]).clamp(0.01, 1.0);
    let mut fh = fw * wf / ratio / hf;
    if fh > 1.0 {
        fw /= fh;
        fh = 1.0;
    }
    place(fw, fh, center)
}

/// Centre du plus grand visage (fractions) ; `None` sans visage ou sans
/// ONNX Runtime.
pub fn subject(img: &RgbImage) -> Option<(f64, f64)> {
    let face = eyes::detector()?.lock().ok()?.largest_face(img)?;
    let (w, h) = (
        f64::from(img.width().max(1)),
        f64::from(img.height().max(1)),
    );
    Some((f64::from(face[0]) / w, f64::from(face[1]) / h))
}

/// Cadrage par défaut pour un visage en `face` (fractions), ou centré.
pub fn default_frame(ratio: f64, w: u32, h: u32, face: Option<(f64, f64)>) -> [f64; 4] {
    let Some((cx, cy)) = face else {
        return fit(ratio, w, h, (0.5, 0.5));
    };
    let [_, top, _, bottom] = fit(ratio, w, h, (0.5, 0.5));
    // Visage aux deux cinquièmes de la hauteur du cadre.
    fit(ratio, w, h, (cx, cy + (bottom - top) * 0.1))
}

/// Cadrages enregistrés d'une photo (identifiant de grille).
pub fn saved(conn: &Connection, item: i64) -> rusqlite::Result<HashMap<String, [f64; 4]>> {
    conn.prepare("SELECT template_id, x0, y0, x1, y1 FROM delivery_framing WHERE item_id = ?1")?
        .query_map([item], |r| {
            Ok((r.get(0)?, [r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?]))
        })?
        .collect()
}

pub fn get(conn: &Connection, item: i64, template: &str) -> rusqlite::Result<Option<[f64; 4]>> {
    conn.query_row(
        "SELECT x0, y0, x1, y1 FROM delivery_framing WHERE item_id = ?1 AND template_id = ?2",
        params![item, template],
        |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?]),
    )
    .optional()
}

pub fn set(conn: &Connection, item: i64, template: &str, rect: [f64; 4]) -> rusqlite::Result<()> {
    let r = rect.map(|v| v.clamp(0.0, 1.0));
    conn.execute(
        "INSERT INTO delivery_framing (item_id, template_id, x0, y0, x1, y1)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (item_id, template_id)
         DO UPDATE SET x0 = ?3, y0 = ?4, x1 = ?5, y1 = ?6",
        params![item, template, r[0], r[1], r[2], r[3]],
    )?;
    Ok(())
}

/// Retour au cadrage par défaut.
pub fn reset(conn: &Connection, item: i64, template: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM delivery_framing WHERE item_id = ?1 AND template_id = ?2",
        params![item, template],
    )?;
    Ok(())
}

/// Cadrage d'une photo pour un modèle, tel que l'interface l'affiche.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Framing {
    pub template: &'static str,
    pub rect: [f64; 4],
    /// Choisi par l'utilisateur (sinon : par défaut).
    pub custom: bool,
    /// Taille de sortie, orientation comprise.
    pub width: u32,
    pub height: u32,
    /// La photo a assez de pixels dans le cadre pour la taille de sortie.
    pub sharp_enough: bool,
}

/// Cadrages d'une photo pour tous les modèles, d'après son aperçu rendu (le
/// cadrage de Développer compris) et sa taille retouchée `full`.
pub fn framings(
    conn: &Connection,
    cache_root: &Path,
    item: i64,
    full: (u32, u32),
) -> rusqlite::Result<Vec<Framing>> {
    let saved = saved(conn, item)?;
    let face = cached_preview(cache_root, item).and_then(|img| subject(&img));
    Ok(TEMPLATES
        .iter()
        .map(|t| {
            let (w, h) = t.size_for(full.0, full.1);
            let ratio = f64::from(w) / f64::from(h);
            let custom = saved.get(t.id);
            let rect = custom.map_or_else(
                || default_frame(ratio, full.0, full.1, face),
                |r| conform(*r, ratio, full.0, full.1),
            );
            let pixels = (rect[2] - rect[0]) * f64::from(full.0);
            Framing {
                template: t.id,
                rect,
                custom: custom.is_some(),
                width: w,
                height: h,
                sharp_enough: pixels + 0.5 >= f64::from(w),
            }
        })
        .collect())
}

/// Point à garder au centre d'une case : le plus grand visage de l'aperçu
/// rendu, sinon le milieu de la photo.
pub fn focus(cache_root: &Path, item: i64) -> [f64; 2] {
    cached_preview(cache_root, item)
        .and_then(|img| subject(&img))
        .map_or([0.5, 0.5], |(x, y)| [x, y])
}

/// Aperçu 1024 du cache (rendu avec les retouches), s'il existe.
fn cached_preview(cache_root: &Path, item: i64) -> Option<RgbImage> {
    let data = std::fs::read(preview::cache_path(cache_root, item, 1024)).ok()?;
    preview::decode_jpeg_scaled(&data, 1024).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 4], b: [f64; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    #[test]
    fn plus_grand_cadre_centre() {
        // Paysage 3:2 vers 4:5 : toute la hauteur, largeur 8/15.
        let r = fit(0.8, 6000, 4000, (0.5, 0.5));
        assert!(
            close(r, [0.5 - 4.0 / 15.0, 0.0, 0.5 + 4.0 / 15.0, 1.0]),
            "{r:?}"
        );
        // Portrait 2:3 vers 16:9 : toute la largeur.
        let r = fit(16.0 / 9.0, 4000, 6000, (0.5, 0.5));
        let fh = 4000.0 / (16.0 / 9.0) / 6000.0;
        assert!(
            close(r, [0.0, 0.5 - fh / 2.0, 1.0, 0.5 + fh / 2.0]),
            "{r:?}"
        );
    }

    #[test]
    fn cadre_tenu_dans_l_image() {
        let r = fit(1.0, 6000, 4000, (0.95, 0.1));
        assert!(close(r, [1.0 - 2.0 / 3.0, 0.0, 1.0, 1.0]), "{r:?}");
    }

    #[test]
    fn visage_un_peu_au_dessus_du_milieu() {
        // Portrait 2:3 vers 16:9 : le visage en haut reste dans le cadre.
        let r = default_frame(16.0 / 9.0, 4000, 6000, Some((0.5, 0.25)));
        assert!(r[1] < 0.25 && r[3] > 0.25, "{r:?}");
        let face_at = (0.25 - r[1]) / (r[3] - r[1]);
        assert!((face_at - 0.4).abs() < 1e-9, "{face_at}");
    }

    #[test]
    fn cadrage_ramene_au_rapport() {
        // Cadre carré enregistré, photo recadrée en 16:9 : la hauteur suit.
        let r = conform([0.2, 0.2, 0.6, 0.8], 1.0, 1600, 900);
        assert!((r[2] - r[0] - 0.4).abs() < 1e-9);
        assert!(((r[3] - r[1]) - 0.4 * 1600.0 / 900.0).abs() < 1e-9, "{r:?}");
        // Trop haut pour l'image : réduit.
        let r = conform([0.0, 0.0, 1.0, 1.0], 1.0, 1600, 900);
        assert!(
            close(r, [0.5 - 900.0 / 3200.0, 0.0, 0.5 + 900.0 / 3200.0, 1.0]),
            "{r:?}"
        );
    }

    #[test]
    fn tirage_suit_l_orientation() {
        let print = find("print-10x15").unwrap();
        assert_eq!(print.size_for(4000, 6000), (1200, 1800));
        assert_eq!(print.size_for(6000, 4000), (1800, 1200));
        let story = find("story").unwrap();
        assert_eq!(story.size_for(6000, 4000), (1080, 1920));
    }

    #[test]
    fn identifiants_uniques() {
        let mut ids: Vec<_> = TEMPLATES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), TEMPLATES.len());
    }
}
