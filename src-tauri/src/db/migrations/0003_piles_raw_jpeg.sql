-- Paires RAW + JPEG en pile (décision 0003). Le fichier du dessous pointe vers
-- celui du dessus ; l'import note le partenaire de chaque fichier de la source.
ALTER TABLE media ADD COLUMN companion_of INTEGER REFERENCES media(id) ON DELETE SET NULL;
CREATE INDEX media_by_companion ON media (companion_of) WHERE companion_of IS NOT NULL;

ALTER TABLE import_items ADD COLUMN pair_item_id INTEGER REFERENCES import_items(id);

-- Paires déjà importées : même dossier, même nom sans extension (casse
-- ignorée), un RAW et un JPEG ou HEIF, prises de vue à 2 secondes près.
-- Les listes d'extensions sont celles de la version 0.1 (metadata/media.rs).
CREATE TEMP TABLE pair_files AS
WITH named AS (
  SELECT id, library_id, captured_at,
         rtrim(path, replace(path, '/', '')) AS folder,
         replace(path, rtrim(path, replace(path, '/', '')), '') AS name
  FROM media
), split AS (
  SELECT id, library_id, captured_at, lower(folder) AS folder,
         lower(replace(name, rtrim(name, replace(name, '.', '')), '')) AS ext,
         lower(substr(name, 1,
           length(name) - length(replace(name, rtrim(name, replace(name, '.', '')), '')) - 1)) AS stem
  FROM named
  WHERE instr(name, '.') > 0
)
SELECT * FROM split;

UPDATE media SET companion_of = (
  SELECT r.id
  FROM pair_files j
  JOIN pair_files r
    ON r.library_id = j.library_id AND r.folder = j.folder AND r.stem = j.stem
  WHERE j.id = media.id
    AND r.ext IN ('3fr', 'arw', 'cr2', 'cr3', 'crw', 'dcr', 'dng', 'erf', 'fff', 'iiq',
                  'k25', 'kdc', 'mef', 'mos', 'mrw', 'nef', 'nrw', 'orf', 'ori', 'pef',
                  'raf', 'raw', 'rw2', 'rwl', 'sr2', 'srf', 'srw', 'x3f')
    AND abs(julianday(r.captured_at) - julianday(j.captured_at)) * 86400 <= 2.5
  ORDER BY r.id
  LIMIT 1
)
WHERE id IN (
  SELECT id FROM pair_files WHERE ext IN ('jpg', 'jpeg', 'heic', 'heif', 'hif')
);

DROP TABLE pair_files;
