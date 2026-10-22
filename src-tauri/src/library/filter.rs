//! Filtres et recherche des grilles : période, note, drapeau, couleur, boîtier,
//! objectif, focale, collection et texte libre. Tout est traduit en SQL
//! paramétré. Un album intelligent enregistre ce filtre (`collections`).

use rusqlite::types::Value;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::LibraryError;

/// Comparaison de la note, comme le filtre d'attributs de Lightroom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RatingOp {
    AtLeast,
    AtMost,
    Exactly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatingFilter {
    pub op: RatingOp,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaSort {
    #[default]
    CaptureTime,
    FileName,
    /// Mieux notées d'abord, puis par date.
    Rating,
}

/// Filtre d'une grille ; un champ absent ne filtre pas. Les champs vides ne
/// sont pas écrits : l'interface lit un champ présent comme un filtre posé.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MediaFilter {
    /// Jour de prise de vue, AAAA-MM-JJ, bornes comprises.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<RatingFilter>,
    /// Drapeaux retenus parmi -1, 0 et 1 ; vide = tous.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<i64>,
    /// Couleurs retenues ; « none » pour les photos sans couleur ; vide = toutes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lens_id: Option<i64>,
    /// Albums de ce client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<i64>,
    /// Albums de ce type de séance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_type_id: Option<i64>,
    /// Texte libre : albums, clients, noms de fichiers, mots-clés, lieux.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Zone tracée sur la carte : ouest, sud, est, nord, en millionièmes de
    /// degré. L'ouest peut dépasser l'est (zone à cheval sur 180°).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<[i64; 4]>,
    /// Focale réelle en millimètres, bornes comprises (une seule focale :
    /// deux fois la même valeur).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focal: Option<[f64; 2]>,
    /// Ratés probables (flou, exposition, yeux fermés) seulement.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub misses: bool,
    /// Collection manuelle : ses photos seulement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<i64>,
}

const COLORS: [&str; 5] = ["red", "yellow", "green", "blue", "purple"];

/// Requête FTS5 sûre : chaque mot devient un préfixe entre guillemets, tous requis.
/// `None` si le texte ne contient aucun mot.
pub fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// Motif LIKE littéral : `%` et `_` échappés par `\`.
fn like_pattern(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn is_day(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

impl MediaFilter {
    /// Clauses SQL (sur l'alias `m`) et leurs paramètres. Les valeurs invalides
    /// sont ignorées plutôt que refusées : un filtre ne doit jamais casser la grille.
    pub fn to_sql(&self) -> (Vec<String>, Vec<Value>) {
        let mut clauses = Vec::new();
        let mut values: Vec<Value> = Vec::new();
        if let Some(from) = self.from.as_deref().filter(|s| is_day(s)) {
            clauses.push("substr(m.captured_at, 1, 10) >= ?".to_owned());
            values.push(from.to_owned().into());
        }
        if let Some(to) = self.to.as_deref().filter(|s| is_day(s)) {
            clauses.push("substr(m.captured_at, 1, 10) <= ?".to_owned());
            values.push(to.to_owned().into());
        }
        if let Some(r) = self.rating {
            let op = match r.op {
                RatingOp::AtLeast => ">=",
                RatingOp::AtMost => "<=",
                RatingOp::Exactly => "=",
            };
            clauses.push(format!("m.rating {op} ?"));
            values.push(i64::from(r.value.min(5)).into());
        }
        let flags: Vec<i64> = self
            .flags
            .iter()
            .copied()
            .filter(|f| (-1..=1).contains(f))
            .collect();
        if !flags.is_empty() {
            clauses.push(format!("m.flag IN ({})", vec!["?"; flags.len()].join(", ")));
            values.extend(flags.into_iter().map(Value::from));
        }
        let colors: Vec<&str> = self
            .colors
            .iter()
            .map(String::as_str)
            .filter(|c| COLORS.contains(c))
            .collect();
        let no_color = self.colors.iter().any(|c| c == "none");
        if !colors.is_empty() || no_color {
            let mut parts = Vec::new();
            if !colors.is_empty() {
                parts.push(format!(
                    "m.color IN ({})",
                    vec!["?"; colors.len()].join(", ")
                ));
                values.extend(colors.into_iter().map(|c| Value::from(c.to_owned())));
            }
            if no_color {
                parts.push("m.color IS NULL".to_owned());
            }
            clauses.push(format!("({})", parts.join(" OR ")));
        }
        if let Some(id) = self.camera_id {
            clauses.push("m.camera_id = ?".to_owned());
            values.push(id.into());
        }
        if let Some(id) = self.lens_id {
            clauses.push("m.lens_id = ?".to_owned());
            values.push(id.into());
        }
        if let Some(id) = self.client_id {
            clauses.push("m.album_id IN (SELECT id FROM albums WHERE client_id = ?)".to_owned());
            values.push(id.into());
        }
        if let Some(id) = self.session_type_id {
            clauses
                .push("m.album_id IN (SELECT id FROM albums WHERE session_type_id = ?)".to_owned());
            values.push(id.into());
        }
        if let Some(text) = self
            .query
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            // L'index retrouve les mots (accents compris) ; le chemin, les
            // morceaux de nom de fichier (« 1234 » dans DSCF1234.RAF).
            let mut parts = vec![
                "m.path LIKE ? ESCAPE '\\'".to_owned(),
                "m.album_id IN (SELECT a.id FROM albums a JOIN clients c ON c.id = a.client_id
                                 WHERE c.name LIKE ? ESCAPE '\\')"
                    .to_owned(),
            ];
            values.push(like_pattern(text).into());
            values.push(like_pattern(text).into());
            if let Some(fts) = fts_query(text) {
                parts.push(
                    "m.id IN (SELECT rowid FROM media_search WHERE media_search MATCH ?)"
                        .to_owned(),
                );
                values.push(fts.into());
            }
            clauses.push(format!("({})", parts.join(" OR ")));
        }
        if let Some([west, south, east, north]) = self.area {
            let deg = |v: i64| v as f64 / 1e6;
            clauses.push("m.lat BETWEEN ? AND ?".to_owned());
            values.push(deg(south.min(north)).into());
            values.push(deg(south.max(north)).into());
            clauses.push(if west <= east {
                "m.lon BETWEEN ? AND ?".to_owned()
            } else {
                "(m.lon >= ? OR m.lon <= ?)".to_owned()
            });
            values.push(deg(west).into());
            values.push(deg(east).into());
        }
        if let Some([a, b]) = self.focal.filter(|f| f.iter().all(|v| v.is_finite())) {
            clauses.push(
                "m.id IN (SELECT media_id FROM media_exif
                           WHERE round(focal_length) BETWEEN ? AND ?)"
                    .to_owned(),
            );
            values.push(a.min(b).round().into());
            values.push(a.max(b).round().into());
        }
        if self.misses {
            clauses.push(
                "m.id IN (SELECT media_id FROM analysis
                           WHERE blurry = 1 OR exposure IS NOT NULL OR eyes_closed = 1)"
                    .to_owned(),
            );
        }
        if let Some(id) = self.collection_id {
            clauses.push(
                "m.id IN (SELECT media_id FROM collection_media WHERE collection_id = ?)"
                    .to_owned(),
            );
            values.push(id.into());
        }
        (clauses, values)
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl MediaSort {
    pub fn order_by(self) -> &'static str {
        match self {
            Self::CaptureTime => "m.captured_at, m.path",
            // Nom du fichier : le chemin privé de ses dossiers.
            Self::FileName => {
                "lower(replace(m.path, rtrim(m.path, replace(m.path, '/', '')), '')), m.path"
            }
            Self::Rating => "m.rating DESC, m.captured_at, m.path",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearOption {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FocalOption {
    /// Focale arrondie au millimètre.
    pub value: f64,
    pub count: i64,
}

/// Choix proposés par la barre de filtres : boîtiers, objectifs et focales présents.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptions {
    pub cameras: Vec<GearOption>,
    pub lenses: Vec<GearOption>,
    pub focals: Vec<FocalOption>,
    /// Premier et dernier jour de prise de vue.
    pub first_day: Option<String>,
    pub last_day: Option<String>,
}

pub fn filter_options(conn: &Connection, library_id: i64) -> Result<FilterOptions, LibraryError> {
    let gear = |column: &str| -> Result<Vec<GearOption>, LibraryError> {
        let mut stmt = conn.prepare(&format!(
            "SELECT g.id, trim(g.make || ' ' || g.model), count(*)
             FROM media m JOIN gear g ON g.id = m.{column}
             WHERE m.library_id = ?1 AND m.state = 'normal' AND g.hidden = 0 AND {}
             GROUP BY g.id ORDER BY 2",
            super::pairs::HEAD_SQL
        ))?;
        let rows = stmt
            .query_map([library_id], |r| {
                Ok(GearOption {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    count: r.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    };
    let focals = conn
        .prepare(&format!(
            "SELECT round(e.focal_length) AS f, count(*)
             FROM media m JOIN media_exif e ON e.media_id = m.id
             WHERE m.library_id = ?1 AND m.state = 'normal' AND e.focal_length > 0 AND {}
             GROUP BY f ORDER BY f",
            super::pairs::HEAD_SQL
        ))?
        .query_map([library_id], |r| {
            Ok(FocalOption {
                value: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let (first_day, last_day) = conn.query_row(
        "SELECT min(substr(captured_at, 1, 10)), max(substr(captured_at, 1, 10))
         FROM media WHERE library_id = ?1 AND state = 'normal'",
        params![library_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(FilterOptions {
        cameras: gear("camera_id")?,
        lenses: gear("lens_id")?,
        focals,
        first_day,
        last_day,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recherche_sans_injection() {
        assert_eq!(fts_query("plage été"), Some("\"plage\"* \"été\"*".into()));
        assert_eq!(fts_query("a\" OR b"), Some("\"a\"* \"OR\"* \"b\"*".into()));
        assert_eq!(fts_query(" -* "), None);
        assert_eq!(like_pattern("100%_x"), "%100\\%\\_x%");
    }

    #[test]
    fn valeurs_invalides_ignorees() {
        let f = MediaFilter {
            from: Some("hier".into()),
            flags: vec![7],
            colors: vec!["rose".into()],
            ..Default::default()
        };
        assert!(f.to_sql().0.is_empty());
        assert!(MediaFilter::default().is_empty());
    }
}
