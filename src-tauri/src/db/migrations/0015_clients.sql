-- Clients et types de séance (cahier des charges, 4.2 ; décision 0009).
-- Un client et un type de séance se rattachent à un album. Les clients sont
-- communs à toutes les bibliothèques ; leurs données restent dans la base,
-- jamais dans les XMP, et la suppression d'un client efface sa fiche.
CREATE TABLE clients (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  email       TEXT,
  phone       TEXT,
  notes       TEXT,
  created_at  TEXT NOT NULL
);

-- Types proposés d'origine : une clé, traduite par l'interface. Types
-- ajoutés par l'utilisateur : le nom saisi. Exactement l'un des deux.
CREATE TABLE session_types (
  id        INTEGER PRIMARY KEY,
  key       TEXT UNIQUE,
  name      TEXT,
  position  INTEGER NOT NULL,
  CHECK ((key IS NULL) <> (name IS NULL))
);

INSERT INTO session_types (key, position) VALUES
  ('wedding', 1), ('baptism', 2), ('birth', 3), ('portrait', 4), ('event', 5),
  ('pregnancy', 6), ('family', 7), ('corporate', 8), ('product', 9), ('sport', 10),
  ('concert', 11);

ALTER TABLE albums ADD COLUMN client_id INTEGER REFERENCES clients(id) ON DELETE SET NULL;
ALTER TABLE albums ADD COLUMN session_type_id INTEGER REFERENCES session_types(id) ON DELETE SET NULL;
CREATE INDEX albums_by_client ON albums (client_id) WHERE client_id IS NOT NULL;
