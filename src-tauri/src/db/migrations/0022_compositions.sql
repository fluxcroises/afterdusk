-- Éditeur de livraison : compositions (une page au format d'un modèle, de
-- une à quatre photos, fond, marge, cadre, textes, logo). Le document est
-- un JSON versionné (`export::design`) ; les photos y sont désignées par
-- leur identifiant de grille, copies virtuelles comprises.
CREATE TABLE delivery_designs (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  document    TEXT NOT NULL,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
