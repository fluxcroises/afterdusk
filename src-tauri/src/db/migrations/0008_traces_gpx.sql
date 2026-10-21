-- Traces GPX qui ont servi à localiser des photos, gardées pour la carte
-- (vue détaillée d'un album). Les points sont en binaire : par point, l'heure
-- en secondes Unix (i64), la latitude et la longitude (f64), petit-boutiste.
CREATE TABLE gpx_tracks (
  id          INTEGER PRIMARY KEY,
  library_id  INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  started_at  TEXT NOT NULL,                -- UTC, RFC 3339
  ended_at    TEXT NOT NULL,
  points      BLOB NOT NULL,
  imported_at TEXT NOT NULL,
  UNIQUE (library_id, started_at, ended_at)
);
CREATE INDEX gpx_by_time ON gpx_tracks (library_id, started_at);
