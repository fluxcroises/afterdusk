-- Retouches non destructives (décision 0005, cahier des charges §5).

-- Une version de photo : l'originale (nom vide) ou une copie virtuelle.
CREATE TABLE edits (
  id         INTEGER PRIMARY KEY,
  media_id   INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  name       TEXT NOT NULL DEFAULT '',          -- '' : l'originale
  settings   TEXT NOT NULL,                     -- JSON versionné (version, process)
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (media_id, name)
);

-- Historique d'une version : chaque étape, instantanés nommés.
CREATE TABLE edit_history (
  id            INTEGER PRIMARY KEY,
  edit_id       INTEGER NOT NULL REFERENCES edits(id) ON DELETE CASCADE,
  position      INTEGER NOT NULL,
  settings      TEXT NOT NULL,
  label         TEXT NOT NULL,                  -- réglage modifié, pour la liste
  snapshot_name TEXT,                           -- non nul : instantané nommé
  created_at    TEXT NOT NULL,
  UNIQUE (edit_id, position)
);

CREATE TABLE presets (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('develop', 'export')),
  settings   TEXT NOT NULL,                     -- JSON creux : seuls les réglages du preset
  builtin    INTEGER NOT NULL DEFAULT 0,        -- fourni avec Afterdusk
  created_at TEXT NOT NULL
);
