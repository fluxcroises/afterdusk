//! Espaces colorimétriques de sortie et leurs profils ICC (moxcms).
//!
//! Le rendu donne des valeurs sRGB codées ; elles sont converties vers
//! l'espace choisi, dont le profil est incorporé au fichier. Le rendu passant
//! par l'espace sRGB (processus 1), Display P3 et Adobe RGB contiennent les
//! mêmes couleurs, exactement décrites dans leur espace.

use moxcms::{ColorProfile, Layout, TransformOptions};
use serde::{Deserialize, Serialize};

use super::ExportError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorSpace {
    #[default]
    Srgb,
    DisplayP3,
    AdobeRgb,
}

impl ColorSpace {
    fn profile(self) -> ColorProfile {
        match self {
            Self::Srgb => ColorProfile::new_srgb(),
            Self::DisplayP3 => ColorProfile::new_display_p3(),
            Self::AdobeRgb => ColorProfile::new_adobe_rgb(),
        }
    }

    /// Profil ICC à incorporer au fichier.
    pub fn icc(self) -> Result<Vec<u8>, ExportError> {
        self.profile()
            .encode()
            .map_err(|e| ExportError::Color(format!("{e:?}")))
    }

    /// Convertit sur place des valeurs sRGB codées (0 à 1, RVB entrelacé)
    /// vers cet espace, codées selon sa courbe.
    pub fn convert(self, rgb: &mut [f32]) -> Result<(), ExportError> {
        if self == Self::Srgb {
            return Ok(());
        }
        let transform = ColorProfile::new_srgb()
            .create_transform_f32(
                Layout::Rgb,
                &self.profile(),
                Layout::Rgb,
                TransformOptions::default(),
            )
            .map_err(|e| ExportError::Color(format!("{e:?}")))?;
        for v in rgb.iter_mut() {
            *v = v.clamp(0.0, 1.0);
        }
        let src = rgb.to_vec();
        transform
            .transform(&src, rgb)
            .map_err(|e| ExportError::Color(format!("{e:?}")))?;
        for v in rgb.iter_mut() {
            *v = v.clamp(0.0, 1.0);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profils_lisibles_et_conversion_juste() {
        for space in [
            ColorSpace::Srgb,
            ColorSpace::DisplayP3,
            ColorSpace::AdobeRgb,
        ] {
            let icc = space.icc().unwrap();
            assert!(icc.len() > 128);
            assert_eq!(&icc[36..40], b"acsp");
            assert!(ColorProfile::new_from_slice(&icc).is_ok());
        }
        // Blanc, noir et gris restent neutres ; un rouge pur sRGB est moins
        // saturé dans un espace plus large.
        let mut px = vec![1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        ColorSpace::AdobeRgb.convert(&mut px).unwrap();
        for v in &px[..3] {
            assert!((v - 1.0).abs() < 0.01, "{px:?}");
        }
        assert!(px[3..6].iter().all(|v| v.abs() < 0.01));
        assert!(px[6] < 0.9 && px[7] < 0.1 && px[8] < 0.1, "{px:?}");
        let mut p3 = vec![1.0, 0.0, 0.0];
        ColorSpace::DisplayP3.convert(&mut p3).unwrap();
        assert!(p3[0] > 0.85 && p3[0] < 0.95 && p3[1] > 0.1, "{p3:?}");
    }
}
