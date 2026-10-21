//! Géocodage inverse hors ligne (décision 0008) : le lieu habité le plus
//! proche de coordonnées, d'après les lieux GeoNames de plus de 1000
//! habitants embarqués dans l'application (`assets/geo/places.bin.z`, format
//! décrit dans `scripts/build-geonames.mjs`). Chargé au premier usage.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

use serde::Serialize;

static DATA: &[u8] = include_bytes!("../../assets/geo/places.bin.z");

/// Au-delà, aucun lieu : photo en mer ou en zone vide.
const MAX_KM: f64 = 60.0;

struct Place {
    lat: f64,
    lon: f64,
    population: u32,
    country: u8,
    region: u16,
    zone: u16,
    name: Box<str>,
}

struct Gazetteer {
    /// Code ISO et nom anglais.
    countries: Vec<([u8; 2], Box<str>)>,
    regions: Vec<Box<str>>,
    zones: Vec<Box<str>>,
    places: Vec<Place>,
    /// Lieux par case d'un degré de côté.
    grid: HashMap<(i32, i32), Vec<u32>>,
}

/// Lieu trouvé.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceName {
    pub city: String,
    pub region: Option<String>,
    /// Code ISO 3166 (« FR »).
    pub country: String,
    /// Nom anglais du pays (recherche).
    pub country_name: String,
    /// Fuseau IANA (« Europe/Paris »).
    pub timezone: String,
    pub latitude: f64,
    pub longitude: f64,
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.data.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn text(&mut self) -> Option<Box<str>> {
        let n = usize::from(self.u8()?);
        Some(std::str::from_utf8(self.take(n)?).ok()?.into())
    }
}

fn cell(lat: f64, lon: f64) -> (i32, i32) {
    (lat.floor() as i32, lon.floor() as i32)
}

fn decode(packed: &[u8]) -> Option<Gazetteer> {
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(packed)
        .read_to_end(&mut raw)
        .ok()?;
    let mut c = Cursor { data: &raw, pos: 0 };
    if c.take(6)? != b"ADGEO1" {
        return None;
    }
    let n = c.u32()?;
    let mut countries = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let code: [u8; 2] = c.take(2)?.try_into().ok()?;
        countries.push((code, c.text()?));
    }
    let n = c.u32()?;
    let mut regions = Vec::with_capacity(n as usize);
    for _ in 0..n {
        c.u16()?;
        regions.push(c.text()?);
    }
    let n = c.u32()?;
    let mut zones = Vec::with_capacity(n as usize);
    for _ in 0..n {
        zones.push(c.text()?);
    }
    let n = c.u32()?;
    let mut places = Vec::with_capacity(n as usize);
    let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
    for i in 0..n {
        let place = Place {
            lat: f64::from(c.i32()?) * 1e-5,
            lon: f64::from(c.i32()?) * 1e-5,
            population: c.u32()?,
            country: c.u8()?,
            region: c.u16()?,
            zone: c.u16()?,
            name: c.text()?,
        };
        grid.entry(cell(place.lat, place.lon)).or_default().push(i);
        places.push(place);
    }
    Some(Gazetteer {
        countries,
        regions,
        zones,
        places,
        grid,
    })
}

fn gazetteer() -> Option<&'static Gazetteer> {
    static CELL: OnceLock<Option<Gazetteer>> = OnceLock::new();
    CELL.get_or_init(|| decode(DATA)).as_ref()
}

/// Distance sur la sphère, en kilomètres.
fn km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * 6371.0 * a.sqrt().asin()
}

impl Gazetteer {
    fn describe(&self, p: &Place) -> PlaceName {
        let (code, name) = &self.countries[usize::from(p.country)];
        PlaceName {
            city: p.name.to_string(),
            region: self
                .regions
                .get(usize::from(p.region))
                .map(|r| r.to_string()),
            country: String::from_utf8_lossy(code).into_owned(),
            country_name: name.to_string(),
            timezone: self.zones[usize::from(p.zone)].to_string(),
            latitude: p.lat,
            longitude: p.lon,
        }
    }

    fn nearest(&self, lat: f64, lon: f64) -> Option<&Place> {
        let (cy, cx) = cell(lat, lon);
        // Trois cases de part et d'autre : plus de 60 km partout sauf près
        // des pôles, où il n'y a guère de lieux.
        let mut best: Option<(f64, &Place)> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                // La longitude fait le tour de -180 à 180.
                let x = (cx + dx + 180).rem_euclid(360) - 180;
                for &i in self.grid.get(&(cy + dy, x)).into_iter().flatten() {
                    let p = &self.places[i as usize];
                    let d = km(lat, lon, p.lat, p.lon);
                    // Une ville s'étend autour de son centre, d'autant plus
                    // qu'elle est peuplée (environ 6 km pour 2 millions
                    // d'habitants) : elle l'emporte sur une commune voisine
                    // dont le centre serait plus proche.
                    let score = d - 0.004 * f64::from(p.population).sqrt();
                    if d <= MAX_KM && best.is_none_or(|(b, _)| score < b) {
                        best = Some((score, p));
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }
}

/// Lieu habité le plus proche, à moins de 60 km.
pub fn nearest(lat: f64, lon: f64) -> Option<PlaceName> {
    let g = gazetteer()?;
    g.nearest(lat, lon).map(|p| g.describe(p))
}

/// Nom anglais d'un pays d'après son code ISO.
pub fn country_name(code: &str) -> Option<String> {
    let g = gazetteer()?;
    g.countries
        .iter()
        .find(|(c, _)| c == code.as_bytes())
        .map(|(_, name)| name.to_string())
}

/// Texte sans accents ni casse, pour la recherche par nom.
fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Lieux dont le nom commence par `query` (sans accents ni casse), les plus
/// peuplés d'abord : la saisie manuelle d'un lieu.
pub fn search(query: &str, limit: usize) -> Vec<PlaceName> {
    let q = fold(query.trim());
    let Some(g) = gazetteer().filter(|_| !q.is_empty()) else {
        return Vec::new();
    };
    let mut found: Vec<&Place> = g
        .places
        .iter()
        .filter(|p| fold(&p.name).starts_with(&q))
        .collect();
    // Nom exact d'abord, puis par population.
    found.sort_by_key(|p| (fold(&p.name) != q, std::cmp::Reverse(p.population)));
    found
        .into_iter()
        .take(limit)
        .map(|p| g.describe(p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lieux_connus() {
        let cabourg = nearest(49.2869, -0.1166).unwrap();
        assert_eq!(cabourg.city, "Cabourg");
        assert_eq!(cabourg.country, "FR");
        assert_eq!(cabourg.country_name, "France");
        assert_eq!(cabourg.timezone, "Europe/Paris");
        assert_eq!(cabourg.region.as_deref(), Some("Normandy"));
        // Hémisphère sud, à l'est.
        assert_eq!(nearest(-33.8568, 151.2153).unwrap().country, "AU");
        // Hémisphère sud, à l'ouest.
        assert_eq!(nearest(-22.9519, -43.2105).unwrap().country, "BR");
        // De l'autre côté de 180° : Egvekinot est à -179,12°.
        assert_eq!(nearest(66.32, 179.95).unwrap().city, "Egvekinot");
        // Grandes villes : la ville et non la commune voisine la plus proche
        // de leur centre.
        assert_eq!(nearest(48.8584, 2.2945).unwrap().city, "Paris");
        assert_eq!(nearest(40.7484, -73.9857).unwrap().city, "New York City");
        // Au milieu de l'Atlantique : rien.
        assert_eq!(nearest(30.0, -40.0), None);
    }

    #[test]
    fn recherche_par_nom_sans_accents() {
        let found = search("saint-etienne", 5);
        assert_eq!(found[0].city, "Saint-Étienne");
        let paris = search("Paris", 3);
        assert_eq!(
            (paris[0].city.as_str(), paris[0].country.as_str()),
            ("Paris", "FR")
        );
        assert!(search("   ", 5).is_empty());
    }

    #[test]
    fn rapide_apres_chargement() {
        nearest(0.0, 0.0);
        let t = std::time::Instant::now();
        for i in 0..1000 {
            nearest(45.0 + f64::from(i) * 0.001, 5.0);
        }
        assert!(t.elapsed().as_millis() < 500, "{:?}", t.elapsed());
    }
}
