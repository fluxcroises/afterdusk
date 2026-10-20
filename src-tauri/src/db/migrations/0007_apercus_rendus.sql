-- Aperçus d'une photo retouchée : rendus d'après ses réglages (source
-- 'rendered') plutôt qu'extraits du fichier. SQLite ne modifie pas une
-- contrainte : la table est recréée, son contenu gardé.
CREATE TABLE previews_new (
  media_id   INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  size       INTEGER NOT NULL CHECK (size IN (256, 1024, 2048)),
  width      INTEGER NOT NULL,
  height     INTEGER NOT NULL,
  bytes      INTEGER NOT NULL,
  source     TEXT NOT NULL CHECK (source IN ('embedded', 'decoded', 'rendered')),
  created_at TEXT NOT NULL,
  last_used  TEXT NOT NULL,
  PRIMARY KEY (media_id, size)
);
INSERT INTO previews_new SELECT * FROM previews;
DROP TABLE previews;
ALTER TABLE previews_new RENAME TO previews;
