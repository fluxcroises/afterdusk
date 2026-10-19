-- Instantanés nommés : une table à part, pour qu'ils survivent aux étapes
-- d'historique remplacées après une annulation. La colonne prévue en 0004,
-- jamais utilisée, disparaît.
CREATE TABLE snapshots (
  id         INTEGER PRIMARY KEY,
  edit_id    INTEGER NOT NULL REFERENCES edits(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  settings   TEXT NOT NULL,                     -- JSON versionné
  created_at TEXT NOT NULL,
  UNIQUE (edit_id, name)
);

ALTER TABLE edit_history DROP COLUMN snapshot_name;

-- xmp_pending devient un compteur de modifications (et non plus 0 ou 1) :
-- le fil d'écriture ne retire une photo de la file que si le compteur n'a pas
-- bougé pendant l'écriture.
DROP INDEX media_xmp_pending;
CREATE INDEX media_xmp_pending ON media (xmp_pending) WHERE xmp_pending > 0;
