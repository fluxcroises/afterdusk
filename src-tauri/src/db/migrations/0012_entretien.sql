-- Journal d'entretien du matériel (cahier des charges, 4.8) : nettoyage du
-- capteur, révision, réparation, mise à jour du firmware. Un intervalle en
-- mois, facultatif, programme le rappel suivant pour ce type d'intervention.
CREATE TABLE maintenance (
  id              INTEGER PRIMARY KEY,
  gear_id         INTEGER NOT NULL REFERENCES gear(id) ON DELETE CASCADE,
  date            TEXT NOT NULL,              -- AAAA-MM-JJ
  kind            TEXT NOT NULL
                  CHECK (kind IN ('sensor', 'service', 'repair', 'firmware', 'other')),
  cost_cents      INTEGER,
  currency        TEXT,
  notes           TEXT,
  reminder_months INTEGER CHECK (reminder_months IS NULL OR reminder_months > 0),
  created_at      TEXT NOT NULL
);
CREATE INDEX maintenance_by_gear ON maintenance (gear_id, kind, date);
