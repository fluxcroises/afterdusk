//! Lecture des traces GPX (GPS Exchange Format 1.0 et 1.1) : points datés
//! des traces, et des itinéraires à défaut.

use std::path::Path;

use chrono::{DateTime, Utc};

use super::GeoError;
use super::matching::Fix;

/// Une trace lue : son nom et ses points datés, triés par l'heure.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: String,
    pub points: Vec<Fix>,
}

fn parse(text: &str, fallback_name: &str) -> Result<Track, GeoError> {
    let doc = roxmltree::Document::parse(text).map_err(|e| GeoError::Gpx(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "gpx" {
        return Err(GeoError::Gpx("pas un fichier GPX".into()));
    }
    let name = root
        .descendants()
        .find(|n| n.has_tag_name("name"))
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_name)
        .to_owned();
    let mut points: Vec<Fix> =
        root.descendants()
            .filter(|n| n.has_tag_name("trkpt") || n.has_tag_name("rtept"))
            .filter_map(|n| {
                let latitude = n.attribute("lat")?.trim().parse::<f64>().ok()?;
                let longitude = n.attribute("lon")?.trim().parse::<f64>().ok()?;
                let time = n
                    .children()
                    .find(|c| c.has_tag_name("time"))
                    .and_then(|c| c.text())
                    .and_then(|t| DateTime::parse_from_rfc3339(t.trim()).ok())?
                    .with_timezone(&Utc);
                ((-90.0..=90.0).contains(&latitude) && (-180.0..=180.0).contains(&longitude))
                    .then_some(Fix {
                        time,
                        latitude,
                        longitude,
                    })
            })
            .collect();
    if points.is_empty() {
        return Err(GeoError::Gpx("aucun point daté".into()));
    }
    points.sort_by_key(|p| p.time);
    Ok(Track { name, points })
}

/// Lit un fichier GPX, sans jamais l'ouvrir en écriture.
pub fn read(path: &Path) -> Result<Track, GeoError> {
    let text = std::fs::read_to_string(path)?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    parse(&text, &stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_dates_tries_et_points_sans_heure_ignores() {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="Montre" xmlns="http://www.topografix.com/GPX/1/1">
  <metadata><name>Sortie à Cabourg</name></metadata>
  <trk><trkseg>
    <trkpt lat="49.2900" lon="-0.1200"><ele>3</ele><time>2026-07-14T10:01:00Z</time></trkpt>
    <trkpt lat="49.2869" lon="-0.1166"><time>2026-07-14T10:00:00Z</time></trkpt>
    <trkpt lat="49.3" lon="-0.13"></trkpt>
    <trkpt lat="91" lon="0"><time>2026-07-14T10:02:00Z</time></trkpt>
  </trkseg></trk>
</gpx>"#;
        let track = parse(text, "fichier").unwrap();
        assert_eq!(track.name, "Sortie à Cabourg");
        assert_eq!(track.points.len(), 2);
        assert!(track.points[0].time < track.points[1].time);
        assert_eq!(track.points[0].latitude, 49.2869);
    }

    #[test]
    fn heure_avec_fuseau_et_nom_du_fichier_par_defaut() {
        let text = r#"<gpx version="1.0"><trk><trkseg>
            <trkpt lat="1" lon="2"><time>2026-07-14T12:00:00+02:00</time></trkpt>
        </trkseg></trk></gpx>"#;
        let track = parse(text, "trace").unwrap();
        assert_eq!(track.name, "trace");
        assert_eq!(
            track.points[0].time.to_rfc3339(),
            "2026-07-14T10:00:00+00:00"
        );
    }

    #[test]
    fn refuse_ce_qui_n_est_pas_une_trace() {
        assert!(parse("<kml/>", "x").is_err());
        assert!(parse("<gpx></gpx>", "x").is_err());
        assert!(parse("pas du xml", "x").is_err());
    }
}
