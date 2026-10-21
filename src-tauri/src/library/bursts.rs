//! Rafales empilées (cahier des charges, 4.3) : photos d'un même boîtier
//! prises à moins de 2 secondes d'intervalle et visuellement proches. Une
//! rafale est une chaîne : chaque photo suit la précédente de 2 secondes au
//! plus et lui ressemble (hash perceptuel). Les rafales trouvées par la
//! détection peuvent être recalculées ; celles empilées à la main jamais.

use std::path::Path;

use chrono::NaiveDateTime;
use image::RgbImage;
use image::imageops::{self, FilterType};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use super::{LibraryError, get_library, pairs, paths};
use crate::db::now_utc;
use crate::metadata::media::format_of;
use crate::raw::preview;

/// Écart maximal entre deux photos voisines d'une rafale, en secondes.
pub const MAX_GAP: f64 = 2.0;
/// Bits différents au plus (sur 64) entre deux photos voisines d'une rafale.
pub const MAX_DISTANCE: u32 = 10;
/// Côté de l'image décodée pour le hash : petit, donc rapide.
const HASH_SOURCE: u32 = 256;

/// dHash : l'image réduite à 9 × 8 en niveaux de gris, un bit par paire de
/// pixels voisins (le gauche est-il plus clair ?). Insensible à l'échelle et
/// aux petites variations de compression, sensible au cadrage.
pub fn dhash(img: &RgbImage) -> u64 {
    let gray = imageops::grayscale(img);
    let small = imageops::resize(&gray, 9, 8, FilterType::Triangle);
    let mut hash = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            hash = (hash << 1) | u64::from(left > right);
        }
    }
    hash
}

pub fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Une photo candidate : boîtier, instant (secondes, fractions comprises),
/// hash perceptuel, note et drapeau.
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    pub id: i64,
    pub camera_id: i64,
    pub time: f64,
    pub phash: Option<u64>,
    pub rating: i64,
    pub flag: i64,
}

/// Chaînes de photos d'un même boîtier séparées de `MAX_GAP` au plus,
/// triées par boîtier puis par instant. Le hash n'intervient pas encore.
fn time_chains(shots: &[Shot]) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..shots.len()).collect();
    order.sort_by(|&a, &b| {
        let (x, y) = (&shots[a], &shots[b]);
        x.camera_id
            .cmp(&y.camera_id)
            .then(x.time.total_cmp(&y.time))
            .then(x.id.cmp(&y.id))
    });
    let mut chains: Vec<Vec<usize>> = Vec::new();
    for i in order {
        let joins = chains.last().and_then(|c| c.last()).is_some_and(|&p| {
            shots[p].camera_id == shots[i].camera_id && shots[i].time - shots[p].time <= MAX_GAP
        });
        if joins {
            if let Some(chain) = chains.last_mut() {
                chain.push(i);
            }
        } else {
            chains.push(vec![i]);
        }
    }
    chains
}

/// Rafales (deux photos au moins) parmi des photos dont le hash est connu :
/// une chaîne de temps se coupe là où deux voisines ne se ressemblent plus.
pub fn group(shots: &[Shot]) -> Vec<Vec<i64>> {
    let mut out = Vec::new();
    for chain in time_chains(shots) {
        let mut current: Vec<usize> = Vec::new();
        for i in chain {
            let close = current.last().is_some_and(|&p| {
                matches!((shots[p].phash, shots[i].phash),
                    (Some(a), Some(b)) if distance(a, b) <= MAX_DISTANCE)
            });
            if !close && current.len() >= 2 {
                out.push(current.iter().map(|&k| shots[k].id).collect());
            }
            if !close {
                current.clear();
            }
            current.push(i);
        }
        if current.len() >= 2 {
            out.push(current.iter().map(|&k| shots[k].id).collect());
        }
    }
    out
}

/// Photo du dessus : la mieux notée, puis la retenue, puis la première.
/// (La plus nette passera devant la première avec la détection des ratés.)
pub fn best(shots: &[&Shot]) -> Option<i64> {
    shots
        .iter()
        .min_by(|a, b| {
            b.rating
                .cmp(&a.rating)
                .then((b.flag == 1).cmp(&(a.flag == 1)))
                .then(a.time.total_cmp(&b.time))
                .then(a.id.cmp(&b.id))
        })
        .map(|s| s.id)
}

/// `AAAA-MM-JJTHH:MM:SS` et fractions EXIF (« 806 » → 0,806 s) en secondes.
fn instant(captured_at: &str, subsec: Option<&str>) -> Option<f64> {
    let t = NaiveDateTime::parse_from_str(captured_at, "%Y-%m-%dT%H:%M:%S").ok()?;
    let frac = subsec
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| format!("0.{s}").parse::<f64>().ok())
        .unwrap_or(0.0);
    Some(t.and_utc().timestamp() as f64 + frac)
}

/// Hash d'une photo : enregistré, sinon calculé sur l'aperçu de 256 px en
/// cache s'il vient du boîtier, sinon sur l'original (lu, jamais modifié).
/// Un aperçu retouché ne sert pas : la retouche éloignerait la photo de ses
/// voisines.
fn ensure_phash(conn: &Connection, cache_root: &Path, media_id: i64) -> Option<u64> {
    let (stored, library_id, rel, orientation): (Option<i64>, i64, String, u8) = conn
        .query_row(
            "SELECT phash, library_id, path, orientation FROM media WHERE id = ?1",
            [media_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok()?;
    if let Some(h) = stored {
        return Some(h as u64);
    }
    let cached: Option<String> = conn
        .query_row(
            "SELECT source FROM previews WHERE media_id = ?1 AND size = 256",
            [media_id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    let from_cache = cached
        .filter(|s| s != "rendered")
        .and_then(|_| std::fs::read(preview::cache_path(cache_root, media_id, 256)).ok())
        .and_then(|data| preview::decode_jpeg_scaled(&data, 64).ok());
    let img = match from_cache {
        Some(img) => img,
        None => {
            let lib = get_library(conn, library_id).ok()?;
            let abs = paths::to_absolute(&lib.root_path, &rel).ok()?;
            let format = format_of(&abs)?;
            preview::load(&abs, format, orientation, HASH_SOURCE).ok()?
        }
    };
    let hash = dhash(&img);
    let _ = conn.execute(
        "UPDATE media SET phash = ?2 WHERE id = ?1",
        params![media_id, hash as i64],
    );
    Some(hash)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectReport {
    /// Rafales en place après la détection.
    pub bursts: usize,
    /// Photos rangées dans ces rafales.
    pub photos: usize,
}

/// Recalcule les rafales automatiques d'une bibliothèque (ou d'un album).
/// Les rafales empilées à la main ne sont pas touchées ; la photo du dessus
/// choisie par l'utilisateur reste au-dessus si elle est toujours dans la
/// rafale. `progress(fait, total)` suit le calcul des hash.
pub fn detect(
    conn: &mut Connection,
    cache_root: &Path,
    library_id: i64,
    album_id: Option<i64>,
    mut progress: impl FnMut(usize, usize),
) -> Result<DetectReport, LibraryError> {
    let mut shots: Vec<Shot> = conn
        .prepare(&format!(
            "SELECT m.id, m.camera_id, m.captured_at,
                    CAST(json_extract(e.raw_json, '$.\"exif.0x9291\"') AS TEXT),
                    m.phash, m.rating, m.flag
               FROM media m LEFT JOIN media_exif e ON e.media_id = m.id
              WHERE m.library_id = ?1 AND (?2 IS NULL OR m.album_id = ?2)
                AND m.state = 'normal' AND m.kind = 'photo' AND {}
                AND m.camera_id IS NOT NULL AND m.captured_at IS NOT NULL
                AND (m.burst_id IS NULL
                     OR m.burst_id IN (SELECT id FROM bursts WHERE automatic = 1))",
            pairs::HEAD_SQL
        ))?
        .query_map(params![library_id, album_id], |r| {
            let at: String = r.get(2)?;
            let subsec: Option<String> = r.get(3)?;
            let (id, camera_id) = (r.get(0)?, r.get(1)?);
            let phash = r.get::<_, Option<i64>>(4)?.map(|h| h as u64);
            let (rating, flag) = (r.get(5)?, r.get(6)?);
            Ok(instant(&at, subsec.as_deref()).map(|time| Shot {
                id,
                camera_id,
                time,
                phash,
                rating,
                flag,
            }))
        })?
        .filter_map(|row| row.transpose())
        .collect::<Result<_, _>>()?;

    // Le hash n'est calculé que pour les photos qui ont une voisine proche
    // dans le temps : une bibliothèque sans rafale ne coûte presque rien.
    let needed: Vec<usize> = time_chains(&shots)
        .into_iter()
        .filter(|c| c.len() >= 2)
        .flatten()
        .filter(|&i| shots[i].phash.is_none())
        .collect();
    for (done, &i) in needed.iter().enumerate() {
        shots[i].phash = ensure_phash(conn, cache_root, shots[i].id);
        progress(done + 1, needed.len());
    }
    let groups = group(&shots);

    let tx = conn.transaction()?;
    // Sommets choisis des rafales automatiques actuelles, à reprendre.
    let tops: Vec<i64> = tx
        .prepare(
            "SELECT top_media_id FROM bursts
              WHERE automatic = 1 AND library_id = ?1 AND top_media_id IS NOT NULL",
        )?
        .query_map([library_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for s in &shots {
        tx.execute(
            "UPDATE media SET burst_id = NULL WHERE id = ?1
               AND burst_id IN (SELECT id FROM bursts WHERE automatic = 1)",
            [s.id],
        )?;
    }
    tx.execute(
        "DELETE FROM bursts WHERE automatic = 1 AND library_id = ?1
           AND NOT EXISTS (SELECT 1 FROM media WHERE burst_id = bursts.id)",
        [library_id],
    )?;
    let now = now_utc();
    let mut report = DetectReport::default();
    for ids in &groups {
        let members: Vec<&Shot> = shots.iter().filter(|s| ids.contains(&s.id)).collect();
        let top = ids
            .iter()
            .copied()
            .find(|id| tops.contains(id))
            .or_else(|| best(&members));
        tx.execute(
            "INSERT INTO bursts (library_id, top_media_id, automatic, created_at)
             VALUES (?1, ?2, 1, ?3)",
            params![library_id, top, now],
        )?;
        let burst = tx.last_insert_rowid();
        for id in ids {
            tx.execute(
                "UPDATE media SET burst_id = ?2 WHERE id = ?1",
                params![id, burst],
            )?;
        }
        report.bursts += 1;
        report.photos += ids.len();
    }
    tx.commit()?;
    Ok(report)
}

fn burst_of(conn: &Connection, media_id: i64) -> Result<Option<i64>, LibraryError> {
    Ok(conn
        .query_row(
            "SELECT burst_id FROM media WHERE id = ?1",
            [media_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

/// Une rafale réduite à une photo n'en est plus une.
fn dissolve_small(conn: &Connection, burst: i64) -> Result<(), LibraryError> {
    let left: i64 = conn.query_row(
        "SELECT count(*) FROM media WHERE burst_id = ?1",
        [burst],
        |r| r.get(0),
    )?;
    if left < 2 {
        conn.execute(
            "UPDATE media SET burst_id = NULL WHERE burst_id = ?1",
            [burst],
        )?;
        conn.execute("DELETE FROM bursts WHERE id = ?1", [burst])?;
    }
    Ok(())
}

/// La photo passe au-dessus de sa rafale.
pub fn set_top(conn: &Connection, media_id: i64) -> Result<bool, LibraryError> {
    let Some(burst) = burst_of(conn, media_id)? else {
        return Ok(false);
    };
    conn.execute(
        "UPDATE bursts SET top_media_id = ?2 WHERE id = ?1",
        params![burst, media_id],
    )?;
    Ok(true)
}

/// Retire des photos de leur rafale. Celle-ci devient manuelle : la
/// détection ne la recalcule plus, et n'y remet donc pas les photos retirées.
pub fn unstack(conn: &mut Connection, ids: &[i64]) -> Result<usize, LibraryError> {
    let tx = conn.transaction()?;
    let mut removed = 0;
    for &id in ids {
        let Some(burst) = burst_of(&tx, id)? else {
            continue;
        };
        tx.execute("UPDATE media SET burst_id = NULL WHERE id = ?1", [id])?;
        tx.execute(
            "UPDATE bursts SET automatic = 0,
                    top_media_id = CASE WHEN top_media_id = ?2 THEN NULL ELSE top_media_id END
              WHERE id = ?1",
            params![burst, id],
        )?;
        dissolve_small(&tx, burst)?;
        removed += 1;
    }
    tx.commit()?;
    Ok(removed)
}

/// Empile des photos à la main (deux au moins) ; elles quittent leur
/// rafale précédente. La première de la liste passe au-dessus.
pub fn stack(
    conn: &mut Connection,
    library_id: i64,
    ids: &[i64],
) -> Result<Option<i64>, LibraryError> {
    let mut ids: Vec<i64> = ids.to_vec();
    ids.dedup();
    if ids.len() < 2 {
        return Ok(None);
    }
    let tx = conn.transaction()?;
    let mut previous = Vec::new();
    for &id in &ids {
        if let Some(b) = burst_of(&tx, id)? {
            previous.push(b);
        }
    }
    tx.execute(
        "INSERT INTO bursts (library_id, top_media_id, automatic, created_at)
         VALUES (?1, ?2, 0, ?3)",
        params![library_id, ids[0], now_utc()],
    )?;
    let burst = tx.last_insert_rowid();
    for &id in &ids {
        tx.execute(
            "UPDATE media SET burst_id = ?2 WHERE id = ?1 AND library_id = ?3",
            params![id, burst, library_id],
        )?;
    }
    for b in previous {
        dissolve_small(&tx, b)?;
    }
    dissolve_small(&tx, burst)?;
    let kept: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM bursts WHERE id = ?1)",
        [burst],
        |r| r.get(0),
    )?;
    tx.commit()?;
    Ok(kept.then_some(burst))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(id: i64, camera: i64, time: f64, phash: u64) -> Shot {
        Shot {
            id,
            camera_id: camera,
            time,
            phash: Some(phash),
            rating: 0,
            flag: 0,
        }
    }

    #[test]
    fn chaines_de_temps_et_de_ressemblance() {
        let a = 0xF0F0_F0F0_F0F0_F0F0;
        let b = a ^ 0b111; // 3 bits : proche
        let far = !a; // tout diffère
        let shots = vec![
            shot(1, 1, 100.0, a),
            shot(2, 1, 101.5, b),
            shot(3, 1, 103.4, a),   // 1,9 s après la précédente : même chaîne
            shot(4, 1, 106.0, a),   // 2,6 s : nouvelle chaîne, seule
            shot(5, 2, 100.2, a),   // autre boîtier, seul
            shot(6, 1, 104.0, far), // dans le temps, mais différente
            shot(7, 1, 104.5, far),
        ];
        let groups = group(&shots);
        assert_eq!(groups, vec![vec![1, 2, 3], vec![6, 7]]);
    }

    #[test]
    fn hash_sans_hash_pas_de_rafale() {
        let mut s = vec![shot(1, 1, 0.0, 0), shot(2, 1, 0.5, 0)];
        s[1].phash = None;
        assert!(group(&s).is_empty());
    }

    #[test]
    fn meilleure_photo_au_dessus() {
        let mut s = [shot(1, 1, 0.0, 0), shot(2, 1, 0.1, 0), shot(3, 1, 0.2, 0)];
        assert_eq!(best(&s.iter().collect::<Vec<_>>()), Some(1), "la première");
        s[2].flag = 1;
        assert_eq!(best(&s.iter().collect::<Vec<_>>()), Some(3), "la retenue");
        s[1].rating = 4;
        assert_eq!(
            best(&s.iter().collect::<Vec<_>>()),
            Some(2),
            "la mieux notée"
        );
    }

    #[test]
    fn instants_avec_fractions() {
        let t = instant("2026-07-14T10:00:05", Some("806")).unwrap();
        let u = instant("2026-07-14T10:00:05", Some("06")).unwrap();
        assert!((t - u - 0.746).abs() < 1e-6);
        assert_eq!(
            instant("2026-07-14T10:00:05", Some("abc")),
            instant("2026-07-14T10:00:05", None)
        );
        assert!(instant("pas une date", None).is_none());
    }

    #[test]
    fn dhash_stable_et_discriminant() {
        let gradient = RgbImage::from_fn(64, 48, |x, _| image::Rgb([(x * 4) as u8; 3]));
        let bigger = imageops::resize(&gradient, 128, 96, FilterType::Triangle);
        assert!(distance(dhash(&gradient), dhash(&bigger)) <= 2);
        let flipped = imageops::flip_horizontal(&gradient);
        assert!(distance(dhash(&gradient), dhash(&flipped)) > 32);
    }
}
