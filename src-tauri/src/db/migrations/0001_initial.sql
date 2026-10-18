-- 0001 : schéma initial de la V0.1 (docs/schema-v0.1.md, validé le 2026-10-18).

CREATE TABLE libraries (
  id              INTEGER PRIMARY KEY,
  name            TEXT NOT NULL,
  root_path       TEXT NOT NULL,              -- absolu, dernier connu
  volume_id       TEXT,                       -- n° de série ou UUID du volume, pour retrouver un disque qui a changé de lettre
  backup_path     TEXT,                       -- absolu, NULL si pas de sauvegarde
  folder_template TEXT NOT NULL DEFAULT '{yyyy}/{yyyy}-{mm}-{dd} {album}',
  rename_template TEXT,                       -- NULL : nom d'origine conservé
  is_online       INTEGER NOT NULL DEFAULT 1, -- recalculé au démarrage et au branchement
  last_seen_at    TEXT,
  created_at      TEXT NOT NULL
);

CREATE TABLE albums (
  id             INTEGER PRIMARY KEY,
  library_id     INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  folder         TEXT NOT NULL,               -- relatif à la racine
  start_date     TEXT,                        -- AAAA-MM-JJ
  end_date       TEXT,
  place          TEXT,                        -- saisie libre en V0.1 ; ville, pays et coordonnées en V0.3
  cover_media_id INTEGER REFERENCES media(id) ON DELETE SET NULL,
  notes          TEXT,
  created_at     TEXT NOT NULL,
  UNIQUE (library_id, folder)
);

-- Boîtiers et objectifs rencontrés dans les EXIF, pour les filtres de la V0.1.
-- La gestion du matériel (achat, entretien, fusion) étendra cette table en V0.3.
CREATE TABLE gear (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('body', 'lens')),
  make          TEXT NOT NULL,
  model         TEXT NOT NULL,
  serial        TEXT,
  auto_detected INTEGER NOT NULL DEFAULT 1,
  hidden        INTEGER NOT NULL DEFAULT 0,
  created_at    TEXT NOT NULL
);

CREATE TABLE gear_aliases (
  gear_id    INTEGER NOT NULL REFERENCES gear(id) ON DELETE CASCADE,
  exif_make  TEXT NOT NULL,
  exif_model TEXT NOT NULL,
  serial     TEXT NOT NULL DEFAULT '',        -- deux boîtiers identiques = deux fiches
  PRIMARY KEY (exif_make, exif_model, serial)
);

CREATE TABLE imports (
  id               INTEGER PRIMARY KEY,
  library_id       INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  source_label     TEXT NOT NULL,             -- « Carte SD (E:) », « Dossier Téléchargements »
  source_path      TEXT NOT NULL,
  source_volume_id TEXT,
  status           TEXT NOT NULL CHECK (status IN ('running', 'done', 'interrupted', 'failed')),
  files_total      INTEGER NOT NULL DEFAULT 0,
  files_done       INTEGER NOT NULL DEFAULT 0,
  backup_path      TEXT,
  options          TEXT NOT NULL DEFAULT '{}', -- JSON : album, mots-clés, copyright, sauvegarde (pour la reprise)
  started_at       TEXT NOT NULL,
  finished_at      TEXT
);

-- Journal fichier par fichier : c'est lui qui permet la reprise après coupure.
-- Un fichier n'est 'done' qu'une fois copié, relu, vérifié (et sauvegardé si demandé).
CREATE TABLE import_items (
  id           INTEGER PRIMARY KEY,
  import_id    INTEGER NOT NULL REFERENCES imports(id) ON DELETE CASCADE,
  source_path  TEXT NOT NULL,                 -- absolu sur la carte, lu seulement
  source_size  INTEGER NOT NULL,
  source_mtime TEXT,
  fingerprint  TEXT NOT NULL,
  target_path  TEXT NOT NULL,                 -- relatif à la racine
  status       TEXT NOT NULL CHECK (status IN
                 ('pending', 'copied', 'verified', 'backed_up', 'done', 'duplicate', 'failed')),
  content_hash TEXT,                          -- BLAKE3 de la source, comparé à la copie relue
  error        TEXT,
  media_id     INTEGER REFERENCES media(id) ON DELETE SET NULL,
  updated_at   TEXT NOT NULL
);
CREATE INDEX import_items_by_status ON import_items (import_id, status);

CREATE TABLE media (
  id                INTEGER PRIMARY KEY,
  library_id        INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  album_id          INTEGER NOT NULL REFERENCES albums(id),
  kind              TEXT NOT NULL CHECK (kind IN ('photo', 'video')),
  path              TEXT NOT NULL,            -- relatif, NFC, '/'
  sidecar_path      TEXT,                     -- relatif
  original_name     TEXT NOT NULL,            -- nom sur la carte
  file_size         INTEGER NOT NULL,
  fingerprint       TEXT NOT NULL,            -- détection des doublons, voir plus bas
  content_hash      TEXT NOT NULL,            -- BLAKE3 complet, vérifié à la copie
  captured_at       TEXT,
  utc_offset        TEXT,                     -- '+02:00'
  width             INTEGER,
  height            INTEGER,
  orientation       INTEGER NOT NULL DEFAULT 1,
  lat               REAL,
  lon               REAL,
  location_source   TEXT CHECK (location_source IN ('exif', 'gpx', 'phone', 'manual')),
  camera_id         INTEGER REFERENCES gear(id) ON DELETE SET NULL,
  lens_id           INTEGER REFERENCES gear(id) ON DELETE SET NULL,
  rating            INTEGER NOT NULL DEFAULT 0 CHECK (rating BETWEEN 0 AND 5),
  flag              INTEGER NOT NULL DEFAULT 0 CHECK (flag IN (-1, 0, 1)),
  color             TEXT CHECK (color IN ('red', 'yellow', 'green', 'blue', 'purple')),
  state             TEXT NOT NULL DEFAULT 'normal' CHECK (state IN ('normal', 'trashed', 'missing')),
  trashed_at        TEXT,
  trash_origin_path TEXT,                     -- emplacement à restaurer
  xmp_pending       INTEGER NOT NULL DEFAULT 0, -- écriture XMP groupée en arrière-plan
  import_id         INTEGER REFERENCES imports(id) ON DELETE SET NULL,
  imported_at       TEXT NOT NULL,
  updated_at        TEXT NOT NULL,
  UNIQUE (library_id, path)
);
CREATE INDEX media_by_album    ON media (album_id, captured_at);
CREATE INDEX media_by_date     ON media (library_id, captured_at);
CREATE INDEX media_by_print    ON media (fingerprint);
CREATE INDEX media_by_camera   ON media (camera_id);
CREATE INDEX media_by_lens     ON media (lens_id);
CREATE INDEX media_by_state    ON media (state, trashed_at);
CREATE INDEX media_xmp_pending ON media (xmp_pending) WHERE xmp_pending = 1;

CREATE TABLE media_exif (
  media_id              INTEGER PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE,
  focal_length          REAL,
  focal_length_35mm     INTEGER,
  aperture              REAL,
  shutter               REAL,                 -- secondes
  iso                   REAL,
  exposure_compensation REAL,
  exposure_program      INTEGER,
  flash                 INTEGER,
  body_serial           TEXT,
  shutter_count         INTEGER,
  raw_json              TEXT                  -- toutes les balises relevées par LibRaw
);

CREATE TABLE keywords (
  id   INTEGER PRIMARY KEY,
  word TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE media_keywords (
  media_id   INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  keyword_id INTEGER NOT NULL REFERENCES keywords(id) ON DELETE CASCADE,
  PRIMARY KEY (media_id, keyword_id)
);

CREATE TABLE collections (
  id          INTEGER PRIMARY KEY,
  library_id  INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'manual' CHECK (kind IN ('manual', 'smart')),
  filter_json TEXT,                           -- albums intelligents, V0.3
  created_at  TEXT NOT NULL
);

CREATE TABLE collection_media (
  collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  media_id      INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  position      INTEGER NOT NULL,
  PRIMARY KEY (collection_id, media_id)
);

-- Index du cache d'aperçus (256, 1024, 2048 px) : sert à afficher une
-- bibliothèque débranchée et à purger le cache selon sa taille maximale.
CREATE TABLE previews (
  media_id   INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  size       INTEGER NOT NULL CHECK (size IN (256, 1024, 2048)),
  width      INTEGER NOT NULL,
  height     INTEGER NOT NULL,
  bytes      INTEGER NOT NULL,
  source     TEXT NOT NULL CHECK (source IN ('embedded', 'decoded')),
  created_at TEXT NOT NULL,
  last_used  TEXT NOT NULL,
  PRIMARY KEY (media_id, size)
);

-- Recherche texte : noms d'albums, de fichiers, mots-clés, lieux. Tenue à jour par le cœur.
CREATE VIRTUAL TABLE media_search USING fts5 (
  album, file_name, original_name, keywords, place,
  content = '', tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL                         -- JSON
);
