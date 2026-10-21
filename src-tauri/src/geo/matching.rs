//! Rapprochement par l'heure : la position d'une photo d'après une trace GPX
//! ou d'après des photos de téléphone. Rien n'est lu ni écrit ici.

use chrono::{DateTime, Duration, FixedOffset, NaiveDateTime, TimeZone, Utc};

/// Point daté (trace GPX ou photo de téléphone), en UTC.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fix {
    pub time: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
}

/// Heure d'une photo selon l'horloge du boîtier, et le fuseau qu'il a écrit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    pub id: i64,
    pub local: NaiveDateTime,
    pub offset: Option<FixedOffset>,
}

/// Comment ramener l'heure du boîtier en UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// Le fuseau écrit par le boîtier dans chaque photo.
    Written,
    /// Heure du boîtier = UTC + ce décalage (fuseau et avance de l'horloge).
    Offset(Duration),
}

impl Shot {
    /// L'instant de la prise de vue, ou rien si le fuseau est inconnu.
    pub fn utc(&self, clock: Clock) -> Option<DateTime<Utc>> {
        let offset = match clock {
            Clock::Offset(d) => FixedOffset::east_opt(i32::try_from(d.num_seconds()).ok()?)?,
            Clock::Written => self.offset?,
        };
        offset
            .from_local_datetime(&self.local)
            .single()
            .map(|t| t.with_timezone(&Utc))
    }
}

/// « +02:00 », « -05:30 » ou « Z ».
pub fn parse_offset(text: &str) -> Option<FixedOffset> {
    let text = text.trim();
    if text == "Z" {
        return FixedOffset::east_opt(0);
    }
    let sign = match text.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let (h, m) = text[1..].split_once(':')?;
    let seconds = h.parse::<i32>().ok()? * 3600 + m.parse::<i32>().ok()? * 60;
    FixedOffset::east_opt(sign * seconds)
}

/// `AAAA-MM-JJTHH:MM:SS` (colonne `media.captured_at`).
pub fn parse_local(text: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(text.get(..19)?, "%Y-%m-%dT%H:%M:%S").ok()
}

fn lerp(a: f64, b: f64, k: f64) -> f64 {
    a + (b - a) * k
}

/// Position sur une trace triée par l'heure : interpolée entre les deux
/// points qui encadrent `t` s'ils sont à moins de `max_gap` l'un de l'autre,
/// sinon le point le plus proche s'il est à moins de `max_gap` de `t`.
pub fn on_track(track: &[Fix], t: DateTime<Utc>, max_gap: Duration) -> Option<(f64, f64)> {
    let after = track.partition_point(|p| p.time < t);
    let before = after.checked_sub(1).map(|i| &track[i]);
    let next = track.get(after);
    if let (Some(a), Some(b)) = (before, next)
        && b.time - a.time <= max_gap
    {
        let span = (b.time - a.time).num_milliseconds();
        let k = if span == 0 {
            0.0
        } else {
            (t - a.time).num_milliseconds() as f64 / span as f64
        };
        return Some((
            lerp(a.latitude, b.latitude, k),
            lerp(a.longitude, b.longitude, k),
        ));
    }
    [before, next]
        .into_iter()
        .flatten()
        .filter(|p| (p.time - t).abs() <= max_gap)
        .min_by_key(|p| (p.time - t).abs())
        .map(|p| (p.latitude, p.longitude))
}

/// Photo de téléphone la plus proche dans le temps, à moins de `window`.
pub fn nearest(fixes: &[Fix], t: DateTime<Utc>, window: Duration) -> Option<(f64, f64)> {
    fixes
        .iter()
        .filter(|p| (p.time - t).abs() <= window)
        .min_by_key(|p| (p.time - t).abs())
        .map(|p| (p.latitude, p.longitude))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 14, h, m, s).unwrap()
    }

    fn fix(t: DateTime<Utc>, lat: f64, lon: f64) -> Fix {
        Fix {
            time: t,
            latitude: lat,
            longitude: lon,
        }
    }

    #[test]
    fn interpole_sur_la_trace_et_refuse_les_trous() {
        let track = [
            fix(at(10, 0, 0), 49.0, -0.2),
            fix(at(10, 1, 0), 49.1, -0.1),
            // Trou de 50 minutes : GPS éteint.
            fix(at(10, 51, 0), 50.0, 1.0),
        ];
        let gap = Duration::minutes(10);
        let (lat, lon) = on_track(&track, at(10, 0, 30), gap).unwrap();
        assert!((lat - 49.05).abs() < 1e-9 && (lon + 0.15).abs() < 1e-9);
        // Dans le trou, près du dernier point avant : ce point.
        assert_eq!(on_track(&track, at(10, 5, 0), gap), Some((49.1, -0.1)));
        // Au milieu du trou : rien.
        assert_eq!(on_track(&track, at(10, 26, 0), gap), None);
        // Avant le début et après la fin, à moins de 10 minutes.
        assert_eq!(on_track(&track, at(9, 55, 0), gap), Some((49.0, -0.2)));
        assert_eq!(on_track(&track, at(11, 30, 0), gap), None);
        assert_eq!(on_track(&[], at(10, 0, 0), gap), None);
    }

    #[test]
    fn photo_de_telephone_la_plus_proche_dans_la_fenetre() {
        let phone = [fix(at(14, 0, 0), 48.0, 2.0), fix(at(15, 0, 0), 43.0, 5.0)];
        let w = Duration::minutes(30);
        assert_eq!(nearest(&phone, at(14, 20, 0), w), Some((48.0, 2.0)));
        assert_eq!(nearest(&phone, at(14, 40, 0), w), Some((43.0, 5.0)));
        assert_eq!(nearest(&phone, at(16, 0, 0), w), None);
    }

    #[test]
    fn heure_du_boitier_en_utc() {
        let shot = Shot {
            id: 1,
            local: parse_local("2026-07-14T12:00:00").unwrap(),
            offset: parse_offset("+02:00"),
        };
        assert_eq!(shot.utc(Clock::Written), Some(at(10, 0, 0)));
        // Boîtier réglé à l'heure de Paris mais en avance de 3 minutes.
        let clock = Clock::Offset(Duration::minutes(123));
        assert_eq!(shot.utc(clock), Some(at(9, 57, 0)));
        let unknown = Shot {
            offset: None,
            ..shot
        };
        assert_eq!(unknown.utc(Clock::Written), None);
        assert_eq!(parse_offset("-05:30").unwrap().local_minus_utc(), -19800);
        assert_eq!(parse_offset("Z").unwrap().local_minus_utc(), 0);
    }
}
