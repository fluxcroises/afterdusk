-- Fiches du matériel (cahier des charges, 4.8) : tous les types d'appareils
-- et d'accessoires, et les champs de la fiche. SQLite ne modifie pas une
-- contrainte : la table est recréée, son contenu et ses identifiants gardés
-- (les clés étrangères sont coupées pendant les migrations, voir db::migrate).
CREATE TABLE gear_new (
  id             INTEGER PRIMARY KEY,
  kind           TEXT NOT NULL CHECK (kind IN
                   ('body', 'lens', 'flash', 'filter', 'tripod', 'bag', 'battery', 'other')),
  make           TEXT NOT NULL,
  model          TEXT NOT NULL,
  serial         TEXT,
  auto_detected  INTEGER NOT NULL DEFAULT 1,
  hidden         INTEGER NOT NULL DEFAULT 0,
  created_at     TEXT NOT NULL,
  -- Nom choisi par l'utilisateur ; vide : marque et modèle.
  name           TEXT,
  -- Silhouette : mirrorless, dslr, compact (boîtiers) ; prime, zoom (objectifs).
  style          TEXT,
  confirmed      INTEGER NOT NULL DEFAULT 0,
  purchase_date  TEXT,                         -- AAAA-MM-JJ
  purchase_place TEXT,
  price_cents    INTEGER,
  currency       TEXT,
  condition      TEXT,
  warranty_until TEXT,                         -- AAAA-MM-JJ
  notes          TEXT,
  -- Photo de l'appareil : une photo de la bibliothèque, ou une image
  -- déposée sur la fiche et copiée à côté de la base (chemin relatif).
  photo_media_id INTEGER REFERENCES media(id) ON DELETE SET NULL,
  photo_file     TEXT
);
INSERT INTO gear_new (id, kind, make, model, serial, auto_detected, hidden, created_at)
SELECT id, kind, make, model, serial, auto_detected, hidden, created_at FROM gear;
DROP TABLE gear;
ALTER TABLE gear_new RENAME TO gear;
