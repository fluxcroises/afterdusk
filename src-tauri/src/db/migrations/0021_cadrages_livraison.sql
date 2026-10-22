-- Modèles de livraison : cadrage choisi pour une photo et un modèle
-- (Instagram 4:5, tirage 10×15…). Fractions de l'image retouchée, de 0 à 1.
-- `item_id` est l'identifiant de grille : positif pour une photo, négatif
-- pour une copie virtuelle (moins l'identifiant de sa version dans `edits`),
-- d'où des déclencheurs plutôt qu'une clé étrangère.
CREATE TABLE delivery_framing (
  item_id     INTEGER NOT NULL,
  template_id TEXT NOT NULL,
  x0          REAL NOT NULL,                  -- gauche
  y0          REAL NOT NULL,                  -- haut
  x1          REAL NOT NULL,                  -- droite
  y1          REAL NOT NULL,                  -- bas
  PRIMARY KEY (item_id, template_id)
);

CREATE TRIGGER delivery_framing_media AFTER DELETE ON media BEGIN
  DELETE FROM delivery_framing WHERE item_id = OLD.id;
END;

CREATE TRIGGER delivery_framing_copy AFTER DELETE ON edits BEGIN
  DELETE FROM delivery_framing WHERE item_id = -OLD.id;
END;
