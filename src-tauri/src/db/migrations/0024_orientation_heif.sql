-- HEIF : FFmpeg rend déjà l'image dans son sens final. Les photos importées
-- avant la correction gardent les dimensions d'avant rotation et une
-- orientation EXIF que le rendu réappliquerait : dimensions échangées pour
-- un quart de tour, orientation ramenée à 1, et aperçus effacés de la base
-- pour être refaits au démarrage. L'original n'est pas touché.
DELETE FROM previews WHERE media_id IN (
  SELECT id FROM media
  WHERE orientation <> 1
    AND (lower(path) LIKE '%.heic' OR lower(path) LIKE '%.heif' OR lower(path) LIKE '%.hif')
);
UPDATE media SET width = height, height = width
WHERE orientation BETWEEN 5 AND 8
  AND (lower(path) LIKE '%.heic' OR lower(path) LIKE '%.heif' OR lower(path) LIKE '%.hif');
UPDATE media SET orientation = 1
WHERE orientation <> 1
  AND (lower(path) LIKE '%.heic' OR lower(path) LIKE '%.heif' OR lower(path) LIKE '%.hif');
