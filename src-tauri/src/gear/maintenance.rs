//! Journal d'entretien et rappels (cahier des charges, 4.8). Pour chaque
//! appareil et chaque type d'intervention, la plus récente décide du rappel :
//! sa date plus son intervalle. Une intervention sans intervalle n'en
//! programme pas.

use chrono::{Months, NaiveDate};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{GearError, SELECT_ITEM, read_item};
use crate::db::now_utc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaintenanceKind {
    Sensor,
    Service,
    Repair,
    Firmware,
    Other,
}

impl MaintenanceKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::Sensor => "sensor",
            Self::Service => "service",
            Self::Repair => "repair",
            Self::Firmware => "firmware",
            Self::Other => "other",
        }
    }

    fn from_db(v: &str) -> Self {
        match v {
            "sensor" => Self::Sensor,
            "service" => Self::Service,
            "repair" => Self::Repair,
            "firmware" => Self::Firmware,
            _ => Self::Other,
        }
    }
}

/// Une intervention telle que le formulaire l'envoie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceInput {
    /// `AAAA-MM-JJ`.
    pub date: String,
    pub kind: MaintenanceKind,
    pub cost_cents: Option<i64>,
    pub currency: Option<String>,
    pub notes: Option<String>,
    pub reminder_months: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceEntry {
    pub id: i64,
    pub gear_id: i64,
    #[serde(flatten)]
    pub input: MaintenanceInput,
}

/// Prochaine intervention d'un type pour un appareil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub gear_id: i64,
    pub gear_name: String,
    pub kind: MaintenanceKind,
    pub last_date: String,
    pub due_date: String,
    /// Date atteinte ou dépassée.
    pub due: bool,
}

fn parse_date(date: &str) -> Result<NaiveDate, GearError> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| GearError::InvalidDate(date.to_owned()))
}

/// Une intervention vide de sens (intervalle nul, notes blanches) est
/// ramenée à sa forme simple avant d'être enregistrée.
fn clean(input: &MaintenanceInput) -> Result<MaintenanceInput, GearError> {
    parse_date(&input.date)?;
    Ok(MaintenanceInput {
        date: input.date.clone(),
        kind: input.kind,
        cost_cents: input.cost_cents.filter(|c| *c >= 0),
        currency: input.currency.clone().filter(|c| !c.trim().is_empty()),
        notes: input.notes.clone().filter(|n| !n.trim().is_empty()),
        reminder_months: input.reminder_months.filter(|m| *m > 0),
    })
}

fn read_entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<MaintenanceEntry> {
    Ok(MaintenanceEntry {
        id: r.get(0)?,
        gear_id: r.get(1)?,
        input: MaintenanceInput {
            date: r.get(2)?,
            kind: MaintenanceKind::from_db(&r.get::<_, String>(3)?),
            cost_cents: r.get(4)?,
            currency: r.get(5)?,
            notes: r.get(6)?,
            reminder_months: r.get(7)?,
        },
    })
}

const SELECT_ENTRY: &str = "SELECT id, gear_id, date, kind, cost_cents, currency, notes,
    reminder_months FROM maintenance";

/// Le journal d'un appareil, le plus récent d'abord.
pub fn list(conn: &Connection, gear_id: i64) -> Result<Vec<MaintenanceEntry>, GearError> {
    let rows = conn
        .prepare(&format!(
            "{SELECT_ENTRY} WHERE gear_id = ?1 ORDER BY date DESC, id DESC"
        ))?
        .query_map([gear_id], read_entry)?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn add(conn: &Connection, gear_id: i64, input: &MaintenanceInput) -> Result<i64, GearError> {
    super::get(conn, gear_id)?;
    let i = clean(input)?;
    conn.execute(
        "INSERT INTO maintenance (gear_id, date, kind, cost_cents, currency, notes,
                                  reminder_months, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            gear_id,
            i.date,
            i.kind.as_db(),
            i.cost_cents,
            i.currency,
            i.notes,
            i.reminder_months,
            now_utc()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, input: &MaintenanceInput) -> Result<(), GearError> {
    let i = clean(input)?;
    let changed = conn.execute(
        "UPDATE maintenance SET date = ?2, kind = ?3, cost_cents = ?4, currency = ?5,
                notes = ?6, reminder_months = ?7
          WHERE id = ?1",
        params![
            id,
            i.date,
            i.kind.as_db(),
            i.cost_cents,
            i.currency,
            i.notes,
            i.reminder_months
        ],
    )?;
    if changed == 0 {
        return Err(GearError::MaintenanceNotFound(id));
    }
    Ok(())
}

/// Une ligne du journal, sans fichier : elle se supprime directement.
pub fn delete(conn: &Connection, id: i64) -> Result<(), GearError> {
    if conn.execute("DELETE FROM maintenance WHERE id = ?1", [id])? == 0 {
        return Err(GearError::MaintenanceNotFound(id));
    }
    Ok(())
}

/// Rappels programmés, les plus proches d'abord : d'un appareil, ou de tout
/// le matériel visible. `due` compare l'échéance à `today`.
pub fn reminders(
    conn: &Connection,
    gear_id: Option<i64>,
    today: NaiveDate,
) -> Result<Vec<Reminder>, GearError> {
    // La plus récente intervention de chaque type, par appareil.
    let latest: Vec<(i64, String, String, Option<u32>)> = conn
        .prepare(
            "SELECT m.gear_id, m.kind, m.date, m.reminder_months
               FROM maintenance m JOIN gear g ON g.id = m.gear_id
              WHERE (?1 IS NULL AND g.hidden = 0 OR m.gear_id = ?1)
                AND m.id = (SELECT m2.id FROM maintenance m2
                             WHERE m2.gear_id = m.gear_id AND m2.kind = m.kind
                             ORDER BY m2.date DESC, m2.id DESC LIMIT 1)",
        )?
        .query_map([gear_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    for (gid, kind, date, months) in latest {
        let Some(months) = months else { continue };
        let Some(due) = parse_date(&date)
            .ok()
            .and_then(|d| d.checked_add_months(Months::new(months)))
        else {
            continue;
        };
        let name = conn
            .query_row(&format!("{SELECT_ITEM} WHERE g.id = ?1"), [gid], read_item)
            .optional()?
            .map(|g| g.display_name)
            .unwrap_or_default();
        out.push(Reminder {
            gear_id: gid,
            gear_name: name,
            kind: MaintenanceKind::from_db(&kind),
            last_date: date,
            due_date: due.format("%Y-%m-%d").to_string(),
            due: due <= today,
        });
    }
    out.sort_by(|a, b| a.due_date.cmp(&b.due_date).then(a.gear_id.cmp(&b.gear_id)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::gear::{GearKind, create};

    fn entry(date: &str, kind: MaintenanceKind, months: Option<u32>) -> MaintenanceInput {
        MaintenanceInput {
            date: date.into(),
            kind,
            cost_cents: None,
            currency: None,
            notes: Some("  ".into()),
            reminder_months: months,
        }
    }

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn journal_et_rappels() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open(&dir.path().join("a.db")).unwrap();
        let body = create(&conn, GearKind::Body, "Nikon", "Z 6").unwrap();
        let lens = create(&conn, GearKind::Lens, "Nikon", "24-70").unwrap();

        // Nettoyage tous les 6 mois ; le plus récent seul compte.
        let old = add(
            &conn,
            body,
            &entry("2025-01-10", MaintenanceKind::Sensor, Some(6)),
        )
        .unwrap();
        add(
            &conn,
            body,
            &entry("2025-11-10", MaintenanceKind::Sensor, Some(6)),
        )
        .unwrap();
        // Une réparation sans intervalle ne programme rien.
        add(
            &conn,
            body,
            &entry("2026-01-05", MaintenanceKind::Repair, None),
        )
        .unwrap();
        add(
            &conn,
            lens,
            &entry("2024-03-31", MaintenanceKind::Service, Some(24)),
        )
        .unwrap();

        let journal = list(&conn, body).unwrap();
        assert_eq!(journal.len(), 3);
        assert_eq!(journal[0].input.date, "2026-01-05");
        assert_eq!(journal[1].input.notes, None, "notes blanches effacées");

        let all = reminders(&conn, None, day("2026-05-10")).unwrap();
        assert_eq!(all.len(), 2, "{all:#?}");
        // Fin de mois : 31 mars + 24 mois = 31 mars, sans débordement.
        assert_eq!(
            (all[0].gear_id, all[0].due_date.as_str()),
            (lens, "2026-03-31")
        );
        assert!(all[0].due);
        assert_eq!(
            (all[1].kind, all[1].due_date.as_str()),
            (MaintenanceKind::Sensor, "2026-05-10")
        );
        assert!(all[1].due, "échéance le jour même");
        assert_eq!(all[1].gear_name, "Nikon Z 6");
        assert!(!reminders(&conn, Some(body), day("2026-05-09")).unwrap()[0].due);

        // Supprimer le nettoyage récent fait revenir l'ancien, échu depuis longtemps.
        let recent = journal[1].id;
        delete(&conn, recent).unwrap();
        let body_due = reminders(&conn, Some(body), day("2026-05-10")).unwrap();
        assert_eq!(body_due[0].due_date, "2025-07-10");

        // Modification : l'intervalle retiré, plus de rappel pour ce type.
        update(
            &conn,
            old,
            &entry("2025-01-10", MaintenanceKind::Sensor, None),
        )
        .unwrap();
        assert!(
            reminders(&conn, Some(body), day("2026-05-10"))
                .unwrap()
                .is_empty()
        );

        // Matériel masqué : hors des rappels généraux.
        crate::gear::set_hidden(&conn, lens, true).unwrap();
        assert!(
            reminders(&conn, None, day("2026-05-10"))
                .unwrap()
                .is_empty()
        );

        assert!(matches!(
            add(
                &conn,
                body,
                &entry("10/05/2026", MaintenanceKind::Other, None)
            ),
            Err(GearError::InvalidDate(_))
        ));
        assert!(matches!(
            delete(&conn, 999),
            Err(GearError::MaintenanceNotFound(999))
        ));
    }
}
