-- Position courante dans l'historique d'une version (annuler, rétablir,
-- revenir à une étape). 0 : l'original, avant toute retouche.
ALTER TABLE edits ADD COLUMN history_position INTEGER NOT NULL DEFAULT 0;

UPDATE edits
SET history_position = coalesce(
  (SELECT max(position) FROM edit_history WHERE edit_history.edit_id = edits.id), 0);
