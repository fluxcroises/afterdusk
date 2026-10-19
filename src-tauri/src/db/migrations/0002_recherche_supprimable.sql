-- Corbeille : le vidage retire les photos de l'index de recherche. Une table
-- FTS5 sans contenu refuse DELETE sauf avec contentless_delete ; on la recrée
-- et on la remplit à nouveau depuis la base.
DROP TABLE media_search;

CREATE VIRTUAL TABLE media_search USING fts5 (
  album, file_name, original_name, keywords, place,
  content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO media_search (rowid, album, file_name, original_name, keywords, place)
SELECT m.id,
       a.name,
       replace(m.path, rtrim(m.path, replace(m.path, '/', '')), ''),
       m.original_name,
       coalesce((SELECT group_concat(k.word, ' ')
                   FROM media_keywords mk JOIN keywords k ON k.id = mk.keyword_id
                  WHERE mk.media_id = m.id), ''),
       ''
  FROM media m JOIN albums a ON a.id = m.album_id;
