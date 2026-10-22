-- Import automatique d'une carte connue (cahier des charges, 4.1 bis ;
-- décision 0009). Une carte est reconnue par le numéro de série du boîtier
-- lu dans ses photos : la règle tient après un formatage dans le boîtier.
-- Les réglages (bibliothèque, rangement, renommage, sauvegarde, mots-clés,
-- copyright) sont ceux de l'import qui a créé la règle, en JSON versionné.
CREATE TABLE auto_import_rules (
  id            INTEGER PRIMARY KEY,
  body_serial   TEXT NOT NULL UNIQUE,
  camera        TEXT NOT NULL,
  library_id    INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  options       TEXT NOT NULL,
  created_at    TEXT NOT NULL,
  last_used_at  TEXT
);
