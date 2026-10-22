-- Vidéos (cahier des charges, 4.9 ; décision 0012). `media.kind` distingue
-- déjà les photos des vidéos ; une vidéo a en plus sa durée, sa cadence,
-- son codec, et ses fichiers annexes de boîtier (métadonnées XML Sony,
-- vignettes THM, basses définitions, sous-titres de vol), copiés, déplacés
-- et mis à la corbeille avec elle.
CREATE TABLE media_video (
  media_id     INTEGER PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE,
  duration_ms  INTEGER NOT NULL,
  fps          REAL,
  codec        TEXT,
  has_audio    INTEGER NOT NULL DEFAULT 0 CHECK (has_audio IN (0, 1))
);

CREATE TABLE media_annexes (
  id             INTEGER PRIMARY KEY,
  media_id       INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  path           TEXT NOT NULL,             -- relatif à la racine de la bibliothèque
  original_name  TEXT NOT NULL,
  UNIQUE (media_id, path)
);
CREATE INDEX media_annexes_by_media ON media_annexes (media_id);
