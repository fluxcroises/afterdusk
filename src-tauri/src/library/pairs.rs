//! Paires RAW + JPEG (décision 0003) : deux fichiers d'un même déclenchement,
//! présentés en pile. Le fichier du dessous pointe vers celui du dessus par
//! `media.companion_of`.

use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDateTime;
use rusqlite::{Connection, OptionalExtension, params};

use super::LibraryError;
use crate::metadata::media::{MediaFormat, format_of};

/// Formats qui accompagnent un RAW : JPEG et HEIF du boîtier.
pub const COMPANION_EXTENSIONS: &[&str] = &["jpg", "jpeg", "heic", "heif", "hif"];

/// Condition SQL (alias `m`) : la photo est le dessus de sa pile, ou n'en a
/// pas, dans son état (normal ou corbeille). Un fichier jeté seul apparaît
/// donc dans la corbeille, et un fichier resté seul reste visible.
pub const HEAD_SQL: &str = "(m.companion_of IS NULL OR NOT EXISTS \
     (SELECT 1 FROM media t WHERE t.id = m.companion_of AND t.state = m.state))";

/// Nombre de fichiers de la pile dans le même état (alias `m`).
pub const STACK_SIZE_SQL: &str =
    "(1 + (SELECT count(*) FROM media c WHERE c.companion_of = m.id AND c.state = m.state))";

/// Écart maximal entre les heures de prise de vue des deux fichiers.
pub const MAX_GAP_SECONDS: i64 = 2;

/// Un fichier candidat : chemin (dossier et nom), format et heure de prise de vue.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub path: &'a Path,
    pub format: MediaFormat,
    pub captured_at: Option<&'a str>,
}

fn is_companion(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| COMPANION_EXTENSIONS.contains(&e.as_str()))
}

/// Dossier et nom sans extension, sans tenir compte de la casse.
fn key(path: &Path) -> Option<(String, String)> {
    let stem = path.file_stem()?.to_string_lossy().to_lowercase();
    let folder = path
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/").to_lowercase())
        .unwrap_or_default();
    Some((folder, stem))
}

fn parse_time(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim_end_matches('Z');
    NaiveDateTime::parse_from_str(&s[..19.min(s.len())], "%Y-%m-%dT%H:%M:%S").ok()
}

/// Heures de prise de vue égales à `MAX_GAP_SECONDS` près ; sans heure, pas de paire.
pub fn same_moment(a: Option<&str>, b: Option<&str>) -> bool {
    match (a.and_then(parse_time), b.and_then(parse_time)) {
        (Some(a), Some(b)) => (a - b).num_seconds().abs() <= MAX_GAP_SECONDS,
        _ => false,
    }
}

/// Paires parmi des fichiers : (indice du RAW, indice du compagnon). Un RAW peut
/// avoir plusieurs compagnons (JPEG et HEIF) ; un compagnon n'a qu'un RAW.
pub fn find_pairs(files: &[Candidate<'_>]) -> Vec<(usize, usize)> {
    let mut raws: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        if f.format == MediaFormat::Raw
            && let Some(k) = key(f.path)
        {
            raws.entry(k).or_default().push(i);
        }
    }
    let mut pairs = Vec::new();
    for (j, f) in files.iter().enumerate() {
        if f.format != MediaFormat::Image || !is_companion(f.path) {
            continue;
        }
        let Some(k) = key(f.path) else { continue };
        let found = raws.get(&k).and_then(|candidates| {
            candidates
                .iter()
                .copied()
                .find(|&i| same_moment(files[i].captured_at, f.captured_at))
        });
        if let Some(i) = found {
            pairs.push((i, j));
        }
    }
    pairs
}

/// Fichier du dessus de la pile d'une photo (elle-même si elle est seule).
pub fn top_of(conn: &Connection, id: i64) -> Result<i64, LibraryError> {
    Ok(conn
        .query_row(
            "SELECT coalesce(companion_of, id) FROM media WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(id))
}

/// Tous les fichiers de la pile, le dessus d'abord, dans l'état donné.
pub fn members(conn: &Connection, id: i64, state: Option<&str>) -> Result<Vec<i64>, LibraryError> {
    let top = top_of(conn, id)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM media WHERE (id = ?1 OR companion_of = ?1)
           AND (?2 IS NULL OR state = ?2)
         ORDER BY companion_of IS NOT NULL, id",
    )?;
    let ids = stmt
        .query_map(params![top, state], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Étend des photos à leurs piles entières (sans doublon, ordre conservé).
pub fn expand(
    conn: &Connection,
    ids: &[i64],
    state: Option<&str>,
) -> Result<Vec<i64>, LibraryError> {
    let mut out: Vec<i64> = Vec::new();
    for &id in ids {
        for m in members(conn, id, state)? {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    }
    Ok(out)
}

/// Fichiers d'un format dans des piles, pour les envoyer seuls à la corbeille
/// (« Envoyer le JPEG dans la corbeille ») : l'autre fichier reste toujours.
/// Une photo seule, ou une pile dont tous les fichiers ont ce format, n'est
/// pas touchée.
pub fn files_of_format(
    conn: &Connection,
    ids: &[i64],
    format: MediaFormat,
) -> Result<Vec<i64>, LibraryError> {
    let mut chosen = Vec::new();
    for &id in ids {
        let members = members(conn, id, Some("normal"))?;
        let mut matching = Vec::new();
        for &m in &members {
            let path: String =
                conn.query_row("SELECT path FROM media WHERE id = ?1", [m], |r| r.get(0))?;
            if format_of(Path::new(&path)) == Some(format) {
                matching.push(m);
            }
        }
        if !matching.is_empty() && matching.len() < members.len() {
            for m in matching {
                if !chosen.contains(&m) {
                    chosen.push(m);
                }
            }
        }
    }
    Ok(chosen)
}

/// Relie un compagnon à son RAW, qui passe au-dessus de la pile.
pub fn link(conn: &Connection, raw: i64, companion: i64) -> Result<(), LibraryError> {
    conn.execute(
        "UPDATE media SET companion_of = ?1 WHERE id = ?2 AND id <> ?1",
        params![raw, companion],
    )?;
    Ok(())
}

/// Met une photo en haut de sa pile ; les autres fichiers passent dessous.
pub fn set_top(conn: &mut Connection, id: i64) -> Result<(), LibraryError> {
    let top = top_of(conn, id)?;
    if top == id {
        return Ok(());
    }
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE media SET companion_of = ?1 WHERE (id = ?2 OR companion_of = ?2) AND id <> ?1",
        params![id, top],
    )?;
    tx.execute("UPDATE media SET companion_of = NULL WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(())
}

/// Le dessus de la pile quitte la grille seul (corbeille) : le premier fichier
/// resté visible prend sa place, et l'ancien dessus pointe vers lui pour
/// retrouver la pile à sa restauration.
pub fn promote_after_leaving(conn: &Connection, left: i64) -> Result<(), LibraryError> {
    let next: Option<i64> = conn
        .query_row(
            "SELECT id FROM media WHERE companion_of = ?1 AND state = 'normal'
               AND EXISTS (SELECT 1 FROM media l WHERE l.id = ?1 AND l.state <> 'normal')
             ORDER BY id LIMIT 1",
            [left],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(next) = next {
        conn.execute(
            "UPDATE media SET companion_of = ?1 WHERE (companion_of = ?2 OR id = ?2) AND id <> ?1",
            params![next, left],
        )?;
        conn.execute("UPDATE media SET companion_of = NULL WHERE id = ?1", [next])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c<'a>(path: &'a str, at: Option<&'a str>) -> Candidate<'a> {
        Candidate {
            path: Path::new(path),
            format: format_of(Path::new(path)).unwrap_or(MediaFormat::Image),
            captured_at: at,
        }
    }

    #[test]
    fn reconnaissance_des_paires() {
        let t = Some("2026-10-19T10:00:00");
        let files = [
            c("DCIM/100/DSC_0001.NEF", t),
            c("DCIM/100/dsc_0001.jpg", Some("2026-10-19T10:00:01")),
            c("DCIM/100/DSC_0002.NEF", t),
            // Compteur repassé par le même numéro : heures trop éloignées.
            c("DCIM/100/DSC_0002.JPG", Some("2026-10-19T10:00:05")),
            // Autre dossier.
            c("DCIM/101/DSC_0003.JPG", t),
            c("DCIM/100/DSC_0003.NEF", t),
            // RAW + HEIF.
            c("DCIM/100/R5_0004.CR3", t),
            c("DCIM/100/R5_0004.HIF", t),
            // JPEG seul, PNG homonyme.
            c("DCIM/100/IMG_0005.JPG", t),
            c("DCIM/100/DSC_0003.PNG", t),
        ];
        assert_eq!(find_pairs(&files), vec![(0, 1), (6, 7)]);
    }

    #[test]
    fn sans_heure_pas_de_paire() {
        assert!(!same_moment(None, Some("2026-10-19T10:00:00")));
        assert!(same_moment(
            Some("2026-10-19T10:00:00Z"),
            Some("2026-10-19T10:00:02")
        ));
    }
}
