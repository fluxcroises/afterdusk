-- Galeries web (cahier des charges, 4.6) : un site statique généré dans un
-- dossier choisi. La base garde le code de chaque galerie et la liste
-- numérotée de ses photos, pour relire la sélection renvoyée par le client
-- même si les fichiers ont été renommés depuis.
CREATE TABLE galleries (
  id          INTEGER PRIMARY KEY,
  library_id  INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  -- « AD-» suivi de 8 caractères, écrit dans la page et dans la sélection.
  code        TEXT NOT NULL UNIQUE,
  title       TEXT NOT NULL,
  album_id    INTEGER REFERENCES albums(id) ON DELETE SET NULL,
  client_id   INTEGER REFERENCES clients(id) ON DELETE SET NULL,
  folder      TEXT NOT NULL,
  created_at  TEXT NOT NULL
);

-- Numéro affiché (1, 2, 3…) et photo ; une copie virtuelle garde son
-- identifiant négatif, sans clé étrangère.
CREATE TABLE gallery_items (
  gallery_id  INTEGER NOT NULL REFERENCES galleries(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  media_id    INTEGER NOT NULL,
  PRIMARY KEY (gallery_id, position)
);
