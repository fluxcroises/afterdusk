//! Matériel (cahier des charges, 4.8) : inventaire tenu à partir des EXIF,
//! fiches complètes, accessoires saisis à la main, fusion des doublons et
//! masquage des appareils d'autrui.

pub mod stats;

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::now_utc;
use crate::import::copy::copy_verified;

#[derive(Debug, thiserror::Error)]
pub enum GearError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("matériel {0} introuvable")]
    NotFound(i64),
    #[error("seuls deux appareils du même type se fusionnent")]
    KindMismatch,
    #[error("un appareil relevé dans les photos se masque, il ne se supprime pas")]
    Detected,
    #[error("photo du matériel : {0}")]
    Photo(String),
}

/// Type d'appareil ou d'accessoire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GearKind {
    Body,
    Lens,
    Flash,
    Filter,
    Tripod,
    Bag,
    Battery,
    Other,
}

impl GearKind {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Lens => "lens",
            Self::Flash => "flash",
            Self::Filter => "filter",
            Self::Tripod => "tripod",
            Self::Bag => "bag",
            Self::Battery => "battery",
            Self::Other => "other",
        }
    }

    fn from_db(v: &str) -> Self {
        match v {
            "body" => Self::Body,
            "lens" => Self::Lens,
            "flash" => Self::Flash,
            "filter" => Self::Filter,
            "tripod" => Self::Tripod,
            "bag" => Self::Bag,
            "battery" => Self::Battery,
            _ => Self::Other,
        }
    }
}

/// Silhouette par défaut d'un appareil relevé dans les EXIF, d'après son nom.
pub fn guess_style(kind: GearKind, make: &str, model: &str) -> Option<&'static str> {
    let m = model.to_uppercase();
    match kind {
        GearKind::Lens => {
            // « 16-35mm », « 70-200 » : un zoom.
            let zoom = m
                .split(|c: char| !c.is_ascii_digit() && c != '-' && c != '.')
                .any(|part| {
                    let mut it = part.split('-');
                    matches!((it.next(), it.next()), (Some(a), Some(b)) if !a.is_empty() && !b.is_empty())
                });
            Some(if zoom { "zoom" } else { "prime" })
        }
        GearKind::Body => {
            let compact = [
                "GR ",
                "GR3",
                "GR II",
                "X100",
                "RX100",
                "RX1",
                "G7 X",
                "G5 X",
                "LX100",
                "Q2",
                "Q3",
                "COOLPIX",
                "POWERSHOT",
                "TG-",
                "X30",
                "XF10",
            ]
            .iter()
            .any(|p| m.contains(p) || m == p.trim());
            let mk = make.to_uppercase();
            let dslr = (mk.contains("CANON")
                && m.contains("EOS")
                && !m.contains("EOS R")
                && !m.contains("EOS M"))
                || (mk.contains("NIKON") && m.trim_start_matches("NIKON ").starts_with('D'))
                || (mk.contains("PENTAX") || mk.contains("RICOH IMAGING")) && m.contains('K')
                || m.contains("SLT-")
                || m.starts_with("DSLR");
            Some(if compact {
                "compact"
            } else if dslr {
                "dslr"
            } else {
                "mirrorless"
            })
        }
        _ => None,
    }
}

/// Ligne de l'inventaire.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearItem {
    pub id: i64,
    pub kind: GearKind,
    pub make: String,
    pub model: String,
    /// Nom choisi, sinon marque et modèle.
    pub display_name: String,
    pub style: Option<String>,
    pub serial: Option<String>,
    pub auto_detected: bool,
    pub hidden: bool,
    pub confirmed: bool,
    pub photo_media_id: Option<i64>,
    pub has_photo_file: bool,
    /// Photos prises avec (une par pile), première et dernière date.
    pub photo_count: i64,
    pub first_used: Option<String>,
    pub last_used: Option<String>,
}

/// Fiche complète.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearRecord {
    #[serde(flatten)]
    pub item: GearItem,
    pub name: Option<String>,
    pub purchase_date: Option<String>,
    pub purchase_place: Option<String>,
    pub price_cents: Option<i64>,
    pub currency: Option<String>,
    pub condition: Option<String>,
    pub warranty_until: Option<String>,
    pub notes: Option<String>,
    /// Noms relevés dans les EXIF (marque, modèle, numéro de série).
    pub aliases: Vec<String>,
}

/// Champs modifiables d'une fiche. Marque, modèle et type ne changent que
/// pour un accessoire saisi à la main.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GearUpdate {
    pub kind: Option<GearKind>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub name: Option<String>,
    pub style: Option<String>,
    pub serial: Option<String>,
    pub purchase_date: Option<String>,
    pub purchase_place: Option<String>,
    pub price_cents: Option<i64>,
    pub currency: Option<String>,
    pub condition: Option<String>,
    pub warranty_until: Option<String>,
    pub notes: Option<String>,
}

const SELECT_ITEM: &str = "
    SELECT g.id, g.kind, g.make, g.model, g.name, g.style, g.serial, g.auto_detected,
           g.hidden, g.confirmed, g.photo_media_id, g.photo_file IS NOT NULL,
           (SELECT count(*) FROM media m
             WHERE (m.camera_id = g.id OR m.lens_id = g.id)
               AND m.state = 'normal' AND m.companion_of IS NULL),
           (SELECT min(m.captured_at) FROM media m
             WHERE (m.camera_id = g.id OR m.lens_id = g.id) AND m.state = 'normal'),
           (SELECT max(m.captured_at) FROM media m
             WHERE (m.camera_id = g.id OR m.lens_id = g.id) AND m.state = 'normal')
      FROM gear g";

fn read_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<GearItem> {
    let kind = GearKind::from_db(&r.get::<_, String>(1)?);
    let make: String = r.get(2)?;
    let model: String = r.get(3)?;
    let name: Option<String> = r.get(4)?;
    let style: Option<String> = r.get(5)?;
    let display_name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        // Un objectif porte souvent sa marque dans son modèle.
        if make.is_empty() || model.to_lowercase().starts_with(&make.to_lowercase()) {
            model.clone()
        } else {
            format!("{make} {model}")
        }
    });
    Ok(GearItem {
        id: r.get(0)?,
        style: style.or_else(|| guess_style(kind, &make, &model).map(str::to_owned)),
        kind,
        display_name,
        make,
        model,
        serial: r.get(6)?,
        auto_detected: r.get(7)?,
        hidden: r.get(8)?,
        confirmed: r.get(9)?,
        photo_media_id: r.get(10)?,
        has_photo_file: r.get(11)?,
        photo_count: r.get(12)?,
        first_used: r.get(13)?,
        last_used: r.get(14)?,
    })
}

/// L'inventaire : boîtiers, objectifs puis accessoires, les plus utilisés
/// d'abord ; les appareils masqués seulement sur demande.
pub fn list(conn: &Connection, include_hidden: bool) -> Result<Vec<GearItem>, GearError> {
    let sql = format!(
        "{SELECT_ITEM} WHERE ?1 OR g.hidden = 0
         ORDER BY CASE g.kind WHEN 'body' THEN 0 WHEN 'lens' THEN 1 ELSE 2 END, g.id"
    );
    let mut items: Vec<GearItem> = conn
        .prepare(&sql)?
        .query_map([include_hidden], read_item)?
        .collect::<Result<_, _>>()?;
    items.sort_by(|a, b| {
        let group = |g: &GearItem| match g.kind {
            GearKind::Body => 0,
            GearKind::Lens => 1,
            _ => 2,
        };
        group(a)
            .cmp(&group(b))
            .then(b.photo_count.cmp(&a.photo_count))
            .then(a.display_name.cmp(&b.display_name))
    });
    Ok(items)
}

pub fn get(conn: &Connection, id: i64) -> Result<GearRecord, GearError> {
    let item = conn
        .query_row(&format!("{SELECT_ITEM} WHERE g.id = ?1"), [id], read_item)
        .optional()?
        .ok_or(GearError::NotFound(id))?;
    let row = conn.query_row(
        "SELECT name, purchase_date, purchase_place, price_cents, currency, condition,
                warranty_until, notes FROM gear WHERE id = ?1",
        [id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        },
    )?;
    let aliases = conn
        .prepare(
            "SELECT trim(exif_make || ' ' || exif_model || CASE WHEN serial = '' THEN ''
                    ELSE ' (' || serial || ')' END)
               FROM gear_aliases WHERE gear_id = ?1 ORDER BY exif_make, exif_model",
        )?
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(GearRecord {
        item,
        name: row.0,
        purchase_date: row.1,
        purchase_place: row.2,
        price_cents: row.3,
        currency: row.4,
        condition: row.5,
        warranty_until: row.6,
        notes: row.7,
        aliases,
    })
}

fn blank(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// Enregistre une fiche (tous ses champs : un champ vide est effacé).
pub fn update(conn: &Connection, id: i64, u: GearUpdate) -> Result<GearRecord, GearError> {
    let current = get(conn, id)?;
    let manual = !current.item.auto_detected;
    let kind = if manual {
        u.kind.unwrap_or(current.item.kind)
    } else {
        current.item.kind
    };
    let make = if manual {
        blank(u.make).unwrap_or_default()
    } else {
        current.item.make.clone()
    };
    let model = if manual {
        blank(u.model).unwrap_or(current.item.model.clone())
    } else {
        current.item.model.clone()
    };
    conn.execute(
        "UPDATE gear SET kind = ?2, make = ?3, model = ?4, name = ?5, style = ?6, serial = ?7,
                purchase_date = ?8, purchase_place = ?9, price_cents = ?10, currency = ?11,
                condition = ?12, warranty_until = ?13, notes = ?14
          WHERE id = ?1",
        params![
            id,
            kind.as_db(),
            make,
            model,
            blank(u.name),
            blank(u.style),
            blank(u.serial),
            blank(u.purchase_date),
            blank(u.purchase_place),
            u.price_cents,
            blank(u.currency),
            blank(u.condition),
            blank(u.warranty_until),
            blank(u.notes),
        ],
    )?;
    get(conn, id)
}

/// Accessoire saisi à la main (filtre, trépied, sac, batterie…).
pub fn create(
    conn: &Connection,
    kind: GearKind,
    make: &str,
    model: &str,
) -> Result<i64, GearError> {
    conn.execute(
        "INSERT INTO gear (kind, make, model, auto_detected, confirmed, created_at)
         VALUES (?1, ?2, ?3, 0, 1, ?4)",
        params![kind.as_db(), make.trim(), model.trim(), now_utc()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Supprime un accessoire saisi à la main ; un appareil relevé dans les
/// photos se masque seulement.
pub fn delete(conn: &Connection, data_dir: &Path, id: i64) -> Result<(), GearError> {
    let item = get(conn, id)?.item;
    if item.auto_detected {
        return Err(GearError::Detected);
    }
    remove_photo_file(conn, data_dir, id)?;
    conn.execute("DELETE FROM gear WHERE id = ?1", [id])?;
    Ok(())
}

pub fn set_hidden(conn: &Connection, id: i64, hidden: bool) -> Result<(), GearError> {
    if conn.execute(
        "UPDATE gear SET hidden = ?2 WHERE id = ?1",
        params![id, hidden],
    )? == 0
    {
        return Err(GearError::NotFound(id));
    }
    Ok(())
}

pub fn set_confirmed(conn: &Connection, id: i64, confirmed: bool) -> Result<(), GearError> {
    if conn.execute(
        "UPDATE gear SET confirmed = ?2 WHERE id = ?1",
        params![id, confirmed],
    )? == 0
    {
        return Err(GearError::NotFound(id));
    }
    Ok(())
}

/// Fusionne `source` dans `target` (deux noms du même appareil) : ses noms
/// EXIF et ses photos passent à `target`, dont la fiche est gardée ; les
/// champs vides de `target` reprennent ceux de `source`.
pub fn merge(conn: &mut Connection, source: i64, target: i64) -> Result<GearRecord, GearError> {
    let (a, b) = (get(conn, source)?, get(conn, target)?);
    if source == target || a.item.kind != b.item.kind {
        return Err(GearError::KindMismatch);
    }
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE OR IGNORE gear_aliases SET gear_id = ?2 WHERE gear_id = ?1",
        params![source, target],
    )?;
    tx.execute(
        "UPDATE media SET camera_id = ?2 WHERE camera_id = ?1",
        params![source, target],
    )?;
    tx.execute(
        "UPDATE media SET lens_id = ?2 WHERE lens_id = ?1",
        params![source, target],
    )?;
    tx.execute(
        "UPDATE gear SET
           name = coalesce(name, (SELECT name FROM gear WHERE id = ?1)),
           serial = coalesce(serial, (SELECT serial FROM gear WHERE id = ?1)),
           purchase_date = coalesce(purchase_date, (SELECT purchase_date FROM gear WHERE id = ?1)),
           purchase_place = coalesce(purchase_place, (SELECT purchase_place FROM gear WHERE id = ?1)),
           price_cents = coalesce(price_cents, (SELECT price_cents FROM gear WHERE id = ?1)),
           currency = coalesce(currency, (SELECT currency FROM gear WHERE id = ?1)),
           condition = coalesce(condition, (SELECT condition FROM gear WHERE id = ?1)),
           warranty_until = coalesce(warranty_until, (SELECT warranty_until FROM gear WHERE id = ?1)),
           notes = coalesce(notes, (SELECT notes FROM gear WHERE id = ?1)),
           photo_media_id = coalesce(photo_media_id, (SELECT photo_media_id FROM gear WHERE id = ?1)),
           photo_file = coalesce(photo_file, (SELECT photo_file FROM gear WHERE id = ?1)),
           confirmed = max(confirmed, (SELECT confirmed FROM gear WHERE id = ?1))
         WHERE id = ?2",
        params![source, target],
    )?;
    tx.execute("DELETE FROM gear WHERE id = ?1", [source])?;
    tx.commit()?;
    get(conn, target)
}

/// Photo de l'appareil choisie dans la bibliothèque, ou aucune (`None`).
/// L'image déposée qu'elle remplace est retirée.
pub fn set_photo_media(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    media_id: Option<i64>,
) -> Result<(), GearError> {
    remove_photo_file(conn, data_dir, id)?;
    conn.execute(
        "UPDATE gear SET photo_media_id = ?2, photo_file = NULL WHERE id = ?1",
        params![id, media_id],
    )?;
    Ok(())
}

/// Efface la copie de l'image déposée (une copie d'Afterdusk, jamais
/// l'image d'origine).
fn remove_photo_file(conn: &Connection, data_dir: &Path, id: i64) -> Result<(), GearError> {
    let file: Option<String> = conn
        .query_row("SELECT photo_file FROM gear WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    if let Some(f) = file {
        let _ = std::fs::remove_file(data_dir.join(f));
    }
    Ok(())
}

/// Image déposée sur la fiche : copiée et vérifiée dans `data_dir/gear/`,
/// jamais déplacée ni modifiée à sa source.
pub fn set_photo_file(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    source: &Path,
) -> Result<(), GearError> {
    get(conn, id)?;
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| ["jpg", "jpeg", "png", "webp"].contains(&e.as_str()))
        .ok_or_else(|| GearError::Photo("format non pris en charge".into()))?;
    let dir = data_dir.join("gear");
    std::fs::create_dir_all(&dir).map_err(|e| GearError::Photo(e.to_string()))?;
    let name = format!("{id}-{}.{ext}", chrono::Utc::now().timestamp_millis());
    copy_verified(source, &dir.join(&name)).map_err(|e| GearError::Photo(e.to_string()))?;
    remove_photo_file(conn, data_dir, id)?;
    conn.execute(
        "UPDATE gear SET photo_file = ?2, photo_media_id = NULL WHERE id = ?1",
        params![id, format!("gear/{name}")],
    )?;
    Ok(())
}

/// Octets de l'image déposée sur la fiche.
pub fn photo_file(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
) -> Result<Option<Vec<u8>>, GearError> {
    let file: Option<String> = conn
        .query_row("SELECT photo_file FROM gear WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    Ok(file.and_then(|f| std::fs::read(data_dir.join(f)).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silhouettes_devinees() {
        let s = |k, mk, m| guess_style(k, mk, m);
        assert_eq!(s(GearKind::Lens, "", "FE 16-35mm F2.8 GM"), Some("zoom"));
        assert_eq!(
            s(GearKind::Lens, "", "RF70-200mm F2.8 L IS USM"),
            Some("zoom")
        );
        assert_eq!(s(GearKind::Lens, "", "XF35mmF1.4 R"), Some("prime"));
        assert_eq!(
            s(GearKind::Body, "Canon", "Canon EOS 5D Mark IV"),
            Some("dslr")
        );
        assert_eq!(
            s(GearKind::Body, "Canon", "Canon EOS R6"),
            Some("mirrorless")
        );
        assert_eq!(
            s(GearKind::Body, "NIKON CORPORATION", "NIKON D850"),
            Some("dslr")
        );
        assert_eq!(
            s(GearKind::Body, "NIKON CORPORATION", "NIKON Z 6"),
            Some("mirrorless")
        );
        assert_eq!(
            s(
                GearKind::Body,
                "RICOH IMAGING COMPANY, LTD.",
                "RICOH GR III"
            ),
            Some("compact")
        );
        assert_eq!(s(GearKind::Body, "FUJIFILM", "X100V"), Some("compact"));
        assert_eq!(s(GearKind::Tripod, "", "Befree"), None);
    }
}
