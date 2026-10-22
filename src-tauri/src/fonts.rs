//! Polices des compositions de Livrer : celles livrées avec Afterdusk (SIL
//! OFL 1.1, identiques sous Windows et macOS), puis celles installées sur
//! l'ordinateur. Une police installée n'est que lue pour tracer le texte
//! dans l'image exportée : elle n'est jamais copiée ni livrée.

use std::borrow::Cow;
use std::sync::OnceLock;

use serde::Serialize;

/// Police livrée : nom de famille et fichier.
pub struct Bundled {
    pub family: &'static str,
    pub data: &'static [u8],
}

/// Police par défaut, et de secours quand une police manque.
pub const DEFAULT: &str = "Inter";

/// Polices livrées, dans l'ordre de l'interface.
pub const BUNDLED: &[Bundled] = &[
    Bundled {
        family: "Inter",
        data: include_bytes!("../assets/fonts/InterVariable.ttf"),
    },
    Bundled {
        family: "Caveat",
        data: include_bytes!("../assets/fonts/Caveat.ttf"),
    },
    Bundled {
        family: "Playfair Display",
        data: include_bytes!("../assets/fonts/PlayfairDisplay.ttf"),
    },
    Bundled {
        family: "Cormorant Garamond",
        data: include_bytes!("../assets/fonts/CormorantGaramond.ttf"),
    },
    Bundled {
        family: "Bebas Neue",
        data: include_bytes!("../assets/fonts/BebasNeue.ttf"),
    },
];

/// Polices installées, relevées une fois (quelques dixièmes de seconde).
fn system() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

fn bundled(family: &str) -> Option<&'static Bundled> {
    BUNDLED
        .iter()
        .find(|b| b.family.eq_ignore_ascii_case(family.trim()))
}

/// Familles proposées à l'utilisateur.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontList {
    pub bundled: Vec<&'static str>,
    pub system: Vec<String>,
}

pub fn list() -> FontList {
    let mut system: Vec<String> = system()
        .faces()
        .filter_map(|f| f.families.first().map(|(name, _)| name.clone()))
        // Polices internes de macOS (« .SF NS ») et doublons des livrées.
        .filter(|name| !name.starts_with('.') && bundled(name).is_none())
        .collect();
    system.sort_by_key(|n| n.to_lowercase());
    system.dedup();
    FontList {
        bundled: BUNDLED.iter().map(|b| b.family).collect(),
        system,
    }
}

/// Données d'une police et rang de la face dans le fichier.
pub struct Face {
    pub data: Cow<'static, [u8]>,
    pub index: u32,
}

/// La face de `family` la plus proche de la graisse `weight` ; `None` si la
/// famille n'est ni livrée ni installée.
pub fn load(family: &str, weight: u16) -> Option<Face> {
    if let Some(b) = bundled(family) {
        return Some(Face {
            data: Cow::Borrowed(b.data),
            index: 0,
        });
    }
    let db = system();
    let id = db.query(&fontdb::Query {
        families: &[fontdb::Family::Name(family.trim())],
        weight: fontdb::Weight(weight),
        ..fontdb::Query::default()
    })?;
    db.with_face_data(id, |data, index| Face {
        data: Cow::Owned(data.to_vec()),
        index,
    })
}

/// La police existe sur cet ordinateur (livrée ou installée).
pub fn available(family: &str) -> bool {
    bundled(family).is_some()
        || system().faces().any(|f| {
            f.families
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case(family.trim()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polices_livrees_lisibles() {
        for b in BUNDLED {
            assert!(
                ab_glyph::FontRef::try_from_slice(b.data).is_ok(),
                "{}",
                b.family
            );
            assert!(available(b.family));
        }
        assert!(load("caveat", 400).is_some());
    }

    #[test]
    fn police_inconnue() {
        assert!(load("Police qui n'existe pas", 400).is_none());
        assert!(!available("Police qui n'existe pas"));
        let l = list();
        assert_eq!(l.bundled[0], DEFAULT);
        assert!(!l.system.iter().any(|n| n == "Inter" || n.starts_with('.')));
    }
}
