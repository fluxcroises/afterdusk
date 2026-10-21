-- Heure de prise de vue corrigée d'après le fuseau du lieu (geo::time) :
-- l'heure écrite par le boîtier est gardée ici, la première fois.
ALTER TABLE media ADD COLUMN captured_at_camera TEXT;
