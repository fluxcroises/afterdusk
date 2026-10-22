-- Dossiers surveillés (cahier des charges, 4.1 bis ; décision 0009) : un
-- dossier où le logiciel d'un boîtier ou un transfert sans fil dépose les
-- photos. Chaque fichier complet y est copié (vérifié) dans l'album choisi ;
-- le dossier n'est jamais modifié. Les fichiers déjà présents à
-- l'activation sont ignorés : seuls ceux créés ou modifiés après
-- `active_since` sont importés, y compris ceux arrivés application fermée.
CREATE TABLE watched_folders (
  id            INTEGER PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  library_id    INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  album_name    TEXT NOT NULL,
  -- Réglages en JSON versionné (renommage, sauvegarde, mots-clés…).
  settings      TEXT NOT NULL,
  enabled       INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  active_since  TEXT NOT NULL,
  created_at    TEXT NOT NULL
);
