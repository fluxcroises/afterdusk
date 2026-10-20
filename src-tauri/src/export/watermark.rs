//! Filigrane : texte (police Inter, licence OFL, livrée avec Afterdusk) ou
//! image (logo PNG), placé sur une grille de neuf points, à une taille
//! relative à la largeur de l'image, avec opacité et marge.

use std::path::PathBuf;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, VariableFont, point};
use serde::{Deserialize, Serialize};

use super::ExportError;
use super::render::Rendered;

/// Police du filigrane texte (variable, graisse réglée à 600).
const FONT: &[u8] = include_bytes!("../../assets/fonts/InterVariable.ttf");
const FONT_WEIGHT: f32 = 600.0;

/// Point d'ancrage sur la grille de neuf points.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    #[default]
    BottomRight,
}

impl Anchor {
    /// Position horizontale et verticale : 0 (début), 1 (milieu), 2 (fin).
    fn cell(self) -> (u8, u8) {
        match self {
            Self::TopLeft => (0, 0),
            Self::Top => (1, 0),
            Self::TopRight => (2, 0),
            Self::Left => (0, 1),
            Self::Center => (1, 1),
            Self::Right => (2, 1),
            Self::BottomLeft => (0, 2),
            Self::Bottom => (1, 2),
            Self::BottomRight => (2, 2),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum WatermarkContent {
    /// Texte, blanc ou noir.
    Text { text: String, dark: bool },
    /// Logo PNG (transparence respectée).
    Image { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Watermark {
    pub content: WatermarkContent,
    pub anchor: Anchor,
    /// Largeur du filigrane, en % de la largeur de l'image (1 à 100).
    pub size: f64,
    /// Opacité, de 0 à 100 %.
    pub opacity: f64,
    /// Marge, en % du petit côté de l'image.
    pub margin: f64,
}

/// Filigrane prêt à poser : couleur et transparence par pixel.
struct Stamp {
    width: u32,
    height: u32,
    /// RVB (0 à 1) et alpha (0 à 1), entrelacés.
    rgba: Vec<f32>,
}

fn text_stamp(text: &str, dark: bool, target_width: f32) -> Result<Option<Stamp>, ExportError> {
    let mut font =
        FontRef::try_from_slice(FONT).map_err(|e| ExportError::Watermark(e.to_string()))?;
    font.set_variation(b"wght", FONT_WEIGHT);
    let text = text.trim();
    if text.is_empty() || target_width < 1.0 {
        return Ok(None);
    }
    // Largeur du texte à 100 px, puis l'échelle qui donne la largeur voulue.
    let measure = |scale: PxScale| {
        let f = font.as_scaled(scale);
        let mut x = 0.0f32;
        let mut previous = None;
        for c in text.chars() {
            let id = f.glyph_id(c);
            if let Some(p) = previous {
                x += f.kern(p, id);
            }
            x += f.h_advance(id);
            previous = Some(id);
        }
        x
    };
    let width_at_100 = measure(PxScale::from(100.0));
    if width_at_100 <= 0.0 {
        return Ok(None);
    }
    let scale = PxScale::from((100.0 * target_width / width_at_100).max(4.0));
    let f = font.as_scaled(scale);
    let width = measure(scale).ceil() as u32 + 2;
    let height = (f.ascent() - f.descent()).ceil() as u32 + 2;
    let mut alpha = vec![0.0f32; (width * height) as usize];
    let mut x = 1.0f32;
    let mut previous = None;
    for c in text.chars() {
        let id = f.glyph_id(c);
        if let Some(p) = previous {
            x += f.kern(p, id);
        }
        let glyph = id.with_scale_and_position(scale, point(x, 1.0 + f.ascent()));
        x += f.h_advance(id);
        previous = Some(id);
        if let Some(outlined) = font.outline_glyph(glyph) {
            let b = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                let (px, py) = (
                    b.min.x as i64 + i64::from(gx),
                    b.min.y as i64 + i64::from(gy),
                );
                if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
                    let i = (py as u32 * width + px as u32) as usize;
                    alpha[i] = (alpha[i] + cov).min(1.0);
                }
            });
        }
    }
    let ink = if dark { 0.0 } else { 1.0 };
    let rgba = alpha.iter().flat_map(|a| [ink, ink, ink, *a]).collect();
    Ok(Some(Stamp {
        width,
        height,
        rgba,
    }))
}

fn image_stamp(path: &std::path::Path, target_width: u32) -> Result<Option<Stamp>, ExportError> {
    let logo = image::open(path)
        .map_err(|e| ExportError::Watermark(format!("{} : {e}", path.display())))?
        .to_rgba8();
    if logo.width() == 0 || target_width == 0 {
        return Ok(None);
    }
    let height = (u64::from(logo.height()) * u64::from(target_width) / u64::from(logo.width()))
        .max(1) as u32;
    let resized = image::imageops::resize(
        &logo,
        target_width,
        height,
        image::imageops::FilterType::Lanczos3,
    );
    let rgba = resized
        .pixels()
        .flat_map(|p| p.0.map(|v| f32::from(v) / 255.0))
        .collect();
    Ok(Some(Stamp {
        width: target_width,
        height,
        rgba,
    }))
}

/// Pose le filigrane sur l'image (valeurs sRGB codées).
pub fn apply(img: &mut Rendered, wm: &Watermark) -> Result<(), ExportError> {
    let size = if wm.size.is_finite() {
        wm.size.clamp(1.0, 100.0)
    } else {
        20.0
    };
    let target = (f64::from(img.width) * size / 100.0).round() as u32;
    let stamp = match &wm.content {
        WatermarkContent::Text { text, dark } => text_stamp(text, *dark, target as f32)?,
        WatermarkContent::Image { path } => image_stamp(path, target.min(img.width))?,
    };
    let Some(stamp) = stamp else {
        return Ok(());
    };
    let opacity = (if wm.opacity.is_finite() {
        wm.opacity
    } else {
        60.0
    })
    .clamp(0.0, 100.0)
        / 100.0;
    let margin = (f64::from(img.width.min(img.height))
        * if wm.margin.is_finite() {
            wm.margin.clamp(0.0, 25.0)
        } else {
            3.0
        }
        / 100.0)
        .round() as i64;
    let (cx, cy) = wm.anchor.cell();
    let place = |cell: u8, full: u32, part: u32| -> i64 {
        match cell {
            0 => margin,
            1 => (i64::from(full) - i64::from(part)) / 2,
            _ => i64::from(full) - i64::from(part) - margin,
        }
    };
    let (x0, y0) = (
        place(cx, img.width, stamp.width),
        place(cy, img.height, stamp.height),
    );
    for sy in 0..stamp.height {
        let y = y0 + i64::from(sy);
        if y < 0 || y >= i64::from(img.height) {
            continue;
        }
        for sx in 0..stamp.width {
            let x = x0 + i64::from(sx);
            if x < 0 || x >= i64::from(img.width) {
                continue;
            }
            let s = ((sy * stamp.width + sx) * 4) as usize;
            let a = stamp.rgba[s + 3] * opacity as f32;
            if a <= 0.0 {
                continue;
            }
            let d = ((y as u32 * img.width + x as u32) * 3) as usize;
            for c in 0..3 {
                img.rgb[d + c] = img.rgb[d + c] * (1.0 - a) + stamp.rgba[s + c] * a;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(w: u32, h: u32) -> Rendered {
        Rendered {
            width: w,
            height: h,
            rgb: vec![0.5; (w * h * 3) as usize],
        }
    }

    /// Rectangle (x0, y0, x1, y1) des pixels modifiés.
    fn changed(img: &Rendered) -> Option<(u32, u32, u32, u32)> {
        let mut b: Option<(u32, u32, u32, u32)> = None;
        for y in 0..img.height {
            for x in 0..img.width {
                if (img.rgb[((y * img.width + x) * 3) as usize] - 0.5).abs() > 0.02 {
                    b = Some(match b {
                        None => (x, y, x, y),
                        Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
                    });
                }
            }
        }
        b
    }

    fn text(anchor: Anchor) -> Watermark {
        Watermark {
            content: WatermarkContent::Text {
                text: "© Afterdusk".into(),
                dark: false,
            },
            anchor,
            size: 30.0,
            opacity: 80.0,
            margin: 5.0,
        }
    }

    #[test]
    fn texte_en_bas_a_droite_a_la_bonne_taille() {
        let mut img = gray(400, 300);
        apply(&mut img, &text(Anchor::BottomRight)).unwrap();
        let (x0, y0, x1, y1) = changed(&img).unwrap();
        // 30 % de 400 px de large, marge de 5 % de 300 px.
        let w = x1 - x0 + 1;
        assert!((105..=122).contains(&w), "largeur {w}");
        assert!(x1 <= 400 - 15 && x1 >= 400 - 20, "{x1}");
        assert!(y1 <= 300 - 15 && y1 >= 300 - 30, "{y1}");
        assert!(y0 > 200);
        // Blanc à 80 % : plus clair.
        assert!(img.rgb.iter().copied().fold(0.0f32, f32::max) > 0.85);
    }

    #[test]
    fn les_neuf_points_et_l_opacite() {
        let mut tl = gray(400, 300);
        apply(&mut tl, &text(Anchor::TopLeft)).unwrap();
        let (x0, y0, _, _) = changed(&tl).unwrap();
        assert!(x0 >= 15 && x0 < 25 && y0 >= 15 && y0 < 40, "{x0} {y0}");
        let mut c = gray(400, 300);
        apply(&mut c, &text(Anchor::Center)).unwrap();
        let (x0, y0, x1, y1) = changed(&c).unwrap();
        assert!(((x0 + x1) as i32 / 2 - 200).abs() <= 4);
        assert!(((y0 + y1) as i32 / 2 - 150).abs() <= 8);
        let mut none = gray(100, 100);
        let mut wm = text(Anchor::Center);
        wm.opacity = 0.0;
        apply(&mut none, &wm).unwrap();
        assert!(changed(&none).is_none());
    }

    #[test]
    fn logo_png_avec_transparence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logo.png");
        // Carré rouge opaque à gauche, transparent à droite.
        let logo = image::RgbaImage::from_fn(20, 10, |x, _| {
            if x < 10 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 0])
            }
        });
        logo.save(&path).unwrap();
        let mut img = gray(200, 100);
        let wm = Watermark {
            content: WatermarkContent::Image { path },
            anchor: Anchor::TopLeft,
            size: 20.0,
            opacity: 100.0,
            margin: 0.0,
        };
        apply(&mut img, &wm).unwrap();
        let px = |x: u32, y: u32| {
            let i = ((y * 200 + x) * 3) as usize;
            [img.rgb[i], img.rgb[i + 1], img.rgb[i + 2]]
        };
        assert!(px(5, 5)[0] > 0.95 && px(5, 5)[1] < 0.05);
        assert_eq!(px(35, 5), [0.5, 0.5, 0.5]);
        assert_eq!(px(100, 50), [0.5, 0.5, 0.5]);
    }
}
