-- Rafales empilées (cahier des charges, 4.3) : photos d'un même boîtier
-- prises à moins de 2 secondes d'intervalle et visuellement proches. Une
-- rafale relie des photos (le dessus d'une paire RAW + JPEG pour une paire) ;
-- sa photo du dessus est affichée, les autres suivent quand on la déplie.
-- Elle vit dans la base seulement : une base reconstruite la recalcule.
CREATE TABLE bursts (
  id            INTEGER PRIMARY KEY,
  library_id    INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  top_media_id  INTEGER REFERENCES media(id) ON DELETE SET NULL,
  -- 1 : trouvée par la détection ; 0 : empilée à la main, jamais recalculée.
  automatic     INTEGER NOT NULL DEFAULT 1 CHECK (automatic IN (0, 1)),
  created_at    TEXT NOT NULL
);

ALTER TABLE media ADD COLUMN burst_id INTEGER REFERENCES bursts(id) ON DELETE SET NULL;
-- Hash perceptuel (dHash 64 bits) de l'aperçu non retouché.
ALTER TABLE media ADD COLUMN phash INTEGER;
CREATE INDEX media_by_burst ON media (burst_id) WHERE burst_id IS NOT NULL;
