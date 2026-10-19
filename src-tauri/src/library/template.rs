//! Modèles de rangement (`{yyyy}/{yyyy}-{mm}-{dd} {album}`) et de renommage
//! (`{yyyy}{mm}{dd}_{hh}{min}{ss}_{camera}_{seq:4}`).
//!
//! Jetons : `{yyyy}` `{yy}` `{mm}` `{dd}` `{hh}` `{min}` `{ss}` `{album}`
//! `{camera}` `{original}` (nom d'origine sans extension) et `{seq}` ou
//! `{seq:N}` (numéro dans l'import, sur N chiffres).

use super::paths::sanitize_component;

pub const DEFAULT_FOLDER_TEMPLATE: &str = "{yyyy}/{yyyy}-{mm}-{dd} {album}";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("jeton inconnu dans le modèle : {{{0}}}")]
    UnknownToken(String),
    #[error("accolade non fermée dans le modèle")]
    Unclosed,
    #[error("le modèle produit un nom vide")]
    Empty,
}

/// Valeurs disponibles pour un fichier.
#[derive(Debug, Clone)]
pub struct TemplateValues<'a> {
    /// `AAAA-MM-JJTHH:MM:SS`, heure du boîtier.
    pub captured_at: &'a str,
    pub album: &'a str,
    pub camera: &'a str,
    pub original_stem: &'a str,
    pub sequence: u32,
}

fn token(name: &str, v: &TemplateValues) -> Result<String, TemplateError> {
    let d = v.captured_at.as_bytes();
    let part = |a: usize, b: usize| {
        if d.len() >= b {
            v.captured_at[a..b].to_owned()
        } else {
            "00".to_owned()
        }
    };
    Ok(match name {
        "yyyy" => part(0, 4),
        "yy" => part(2, 4),
        "mm" => part(5, 7),
        "dd" => part(8, 10),
        "hh" => part(11, 13),
        "min" => part(14, 16),
        "ss" => part(17, 19),
        "album" => v.album.to_owned(),
        "camera" => v.camera.replace(' ', "-"),
        "original" => v.original_stem.to_owned(),
        "seq" => v.sequence.to_string(),
        other => {
            let width = other
                .strip_prefix("seq:")
                .and_then(|w| w.parse::<usize>().ok())
                .filter(|w| (1..=9).contains(w))
                .ok_or_else(|| TemplateError::UnknownToken(other.to_owned()))?;
            format!("{:0width$}", v.sequence)
        }
    })
}

/// Rend un segment (sans `/`) : jetons remplacés puis nettoyage du nom.
fn render_segment(segment: &str, v: &TemplateValues) -> Result<String, TemplateError> {
    let mut out = String::new();
    let mut rest = segment;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = rest[start..].find('}').ok_or(TemplateError::Unclosed)? + start;
        out.push_str(&token(&rest[start + 1..end], v)?);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    // Les valeurs (album, boîtier) ne peuvent pas introduire de sous-dossier.
    let clean = sanitize_component(&out);
    if clean.is_empty() {
        Err(TemplateError::Empty)
    } else {
        Ok(clean)
    }
}

/// Dossier relatif, segments séparés par `/`.
pub fn render_folder(template: &str, v: &TemplateValues) -> Result<String, TemplateError> {
    template
        .split('/')
        .filter(|s| !s.trim().is_empty())
        .map(|s| render_segment(s, v))
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("/"))
        .and_then(|p| {
            if p.is_empty() {
                Err(TemplateError::Empty)
            } else {
                Ok(p)
            }
        })
}

/// Nom de fichier ; l'extension d'origine est conservée telle quelle.
pub fn render_file_name(
    template: &str,
    v: &TemplateValues,
    extension: &str,
) -> Result<String, TemplateError> {
    let stem = render_segment(template, v)?;
    Ok(if extension.is_empty() {
        stem
    } else {
        format!("{stem}.{extension}")
    })
}

/// Vérifie un modèle sans fichier (réglages, aperçu du résultat).
pub fn validate(template: &str) -> Result<(), TemplateError> {
    let v = TemplateValues {
        captured_at: "2026-10-18T21:04:59",
        album: "Album",
        camera: "Boîtier",
        original_stem: "IMG_0001",
        sequence: 1,
    };
    render_folder(template, &v).map(|_| ())
}

/// Aperçu d'un rangement et d'un renommage sur une photo d'exemple, pour les
/// réglages et la première ouverture : `2026/2026-10-18 Sortie/…`.
pub fn preview(
    folder_template: &str,
    rename_template: Option<&str>,
    album: &str,
) -> Result<String, TemplateError> {
    let v = TemplateValues {
        captured_at: "2026-10-18T21:04:59",
        album,
        camera: "X-T5",
        original_stem: "DSCF0042",
        sequence: 42,
    };
    let folder = render_folder(folder_template, &v)?;
    let name = match rename_template {
        Some(t) => render_file_name(t, &v, "RAF")?,
        None => "DSCF0042.RAF".to_owned(),
    };
    Ok(format!("{folder}/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> TemplateValues<'static> {
        TemplateValues {
            captured_at: "2026-10-18T21:04:59",
            album: "Sortie à Saint-Malo",
            camera: "X-T3",
            original_stem: "DSCF0001",
            sequence: 7,
        }
    }

    #[test]
    fn rangement_par_defaut() {
        assert_eq!(
            render_folder(DEFAULT_FOLDER_TEMPLATE, &values()).unwrap(),
            "2026/2026-10-18 Sortie à Saint-Malo"
        );
    }

    #[test]
    fn renommage_du_cahier_des_charges() {
        assert_eq!(
            render_file_name(
                "{yyyy}{mm}{dd}_{hh}{min}{ss}_{camera}_{seq:4}",
                &values(),
                "RAF"
            )
            .unwrap(),
            "20261018_210459_X-T3_0007.RAF"
        );
    }

    #[test]
    fn une_valeur_ne_cree_pas_de_sous_dossier() {
        let mut v = values();
        v.album = "Mer/Plage";
        assert_eq!(
            render_folder(DEFAULT_FOLDER_TEMPLATE, &v).unwrap(),
            "2026/2026-10-18 Mer-Plage"
        );
    }

    #[test]
    fn erreurs_de_modele() {
        assert_eq!(
            validate("{annee}"),
            Err(TemplateError::UnknownToken("annee".into()))
        );
        assert_eq!(validate("{yyyy"), Err(TemplateError::Unclosed));
        assert_eq!(validate("/ /"), Err(TemplateError::Empty));
        assert!(validate(DEFAULT_FOLDER_TEMPLATE).is_ok());
    }
}
