-- Dérushage vidéo : extraits d'un clip, à exporter sans réencodage. Le
-- début est recalé sur une image clé (une copie sans réencodage ne peut
-- commencer qu'ainsi) ; l'original n'est jamais modifié.
CREATE TABLE video_clips (
  id          INTEGER PRIMARY KEY,
  media_id    INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  start_ms    INTEGER NOT NULL,
  end_ms      INTEGER NOT NULL,
  name        TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL,
  CHECK (start_ms >= 0 AND end_ms > start_ms)
);

CREATE INDEX video_clips_media ON video_clips (media_id, start_ms);
