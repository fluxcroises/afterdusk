-- Connexion au boîtier (V0.8, décision 0016) : photos déjà reçues du
-- boîtier, pour ne pas les relire quand Afterdusk redémarre câble branché
-- (le boîtier les liste encore). Le boîtier renumérote ses photos à chaque
-- branchement : une photo se reconnaît à son nom, sa date et sa taille.
CREATE TABLE tether_received (
  serial      TEXT NOT NULL,
  name        TEXT NOT NULL,
  captured    TEXT NOT NULL,
  size        INTEGER NOT NULL,
  received_at TEXT NOT NULL,
  PRIMARY KEY (serial, name, captured, size)
);
