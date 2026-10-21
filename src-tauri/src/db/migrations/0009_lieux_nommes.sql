-- Lieu nommé d'une photo localisée, déduit hors ligne de ses coordonnées
-- (geo::places) : ville, région, pays (code ISO) et fuseau IANA. Un pays
-- vide signale une photo cherchée sans lieu habité à proximité (en mer).
ALTER TABLE media ADD COLUMN city TEXT;
ALTER TABLE media ADD COLUMN region TEXT;
ALTER TABLE media ADD COLUMN country TEXT;
ALTER TABLE media ADD COLUMN timezone TEXT;
CREATE INDEX media_unnamed ON media (id) WHERE lat IS NOT NULL AND country IS NULL;
