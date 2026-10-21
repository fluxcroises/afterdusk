-- Détection des ratés (cahier des charges, 4.3 et 5) : une ligne par photo
-- analysée. `version` relance l'analyse quand la méthode change. Les yeux
-- fermés restent vides tant que leur modèle n'est pas adopté. Le hash
-- perceptuel est sur `media` (migration 0013), calculé avec les rafales.
CREATE TABLE analysis (
  media_id    INTEGER PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE,
  version     INTEGER NOT NULL,
  sharpness   REAL NOT NULL,
  blurry      INTEGER NOT NULL CHECK (blurry IN (0, 1)),
  exposure    TEXT CHECK (exposure IN ('under', 'over')),
  eyes_closed INTEGER CHECK (eyes_closed IN (0, 1)),
  analyzed_at TEXT NOT NULL
);
