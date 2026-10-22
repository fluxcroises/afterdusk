-- Dossiers surveillés : relevé des fichiers présents à l'activation, qui
-- sont ignorés. Les dates des fichiers ne suffisent pas : sous macOS, une
-- copie (Finder comme programme) garde les dates d'origine, et une photo
-- déposée après l'activation paraissait déjà là. Chemins relatifs au dossier.
CREATE TABLE watched_existing (
  folder_id  INTEGER NOT NULL REFERENCES watched_folders(id) ON DELETE CASCADE,
  path       TEXT NOT NULL,
  PRIMARY KEY (folder_id, path)
);
