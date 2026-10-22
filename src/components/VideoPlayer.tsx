import { invoke } from "@tauri-apps/api/core";
import {
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  Pause,
  Play,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { previewUrl, videoUrl } from "../api/previews";
import type { MediaSummary } from "../store/library";
import { formatDuration } from "../ui/format";
import { IconButton } from "./IconButton";

/** Miroir de `commands::video::VideoSource`. */
interface VideoSource {
  path: string | null;
  needsProxy: boolean;
  fps: number;
  durationMs: number;
  codec: string;
  ffmpeg: boolean;
}

const CODECS: Record<string, string> = {
  h264: "H.264",
  hevc: "HEVC",
  prores: "ProRes",
  mpeg2video: "MPEG-2",
  av1: "AV1",
};

/**
 * Lecture d'une vidéo dans la loupe : lecture et pause (Espace), image par
 * image (, et .), son, et la ligne d'informations. Un codec que le lecteur
 * intégré ne lit pas passe par une version de lecture préparée en cache.
 */
export function VideoPlayer({ media }: { media: MediaSummary }) {
  const { t } = useTranslation();
  const video = useRef<HTMLVideoElement>(null);
  const [source, setSource] = useState<VideoSource | null>(null);
  const [preparing, setPreparing] = useState(false);
  const [failed, setFailed] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [muted, setMuted] = useState(false);
  const [time, setTime] = useState(0);

  useEffect(() => {
    // Une vidéo par lecteur : la loupe le recrée à chaque média (clé).
    let current = true;
    const load = async () => {
      const s = await invoke<VideoSource>("video_source", { mediaId: media.id });
      if (!current) return;
      setSource(s);
      if (s.needsProxy && s.ffmpeg) {
        setPreparing(true);
        try {
          await invoke("video_prepare", { mediaId: media.id });
          const ready = await invoke<VideoSource>("video_source", { mediaId: media.id });
          if (current) setSource(ready);
        } catch {
          if (current) setFailed(true);
        } finally {
          if (current) setPreparing(false);
        }
      }
    };
    load().catch(() => current && setFailed(true));
    return () => {
      current = false;
    };
  }, [media.id]);

  const fps = source?.fps || 25;
  const toggle = useCallback(() => {
    const v = video.current;
    if (!v) return;
    if (v.paused) void v.play();
    else v.pause();
  }, []);
  const step = useCallback(
    (frames: number) => {
      const v = video.current;
      if (!v) return;
      v.pause();
      v.currentTime = Math.min(Math.max(0, v.currentTime + frames / fps), v.duration || 0);
    },
    [fps],
  );

  // Espace : lecture ; virgule et point : une image en arrière ou en avant.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target instanceof Element ? e.target : null;
      // Le curseur de position garde les raccourcis ; un vrai champ, non.
      const inField = target?.closest("input:not([type=range]), textarea, select") != null;
      if (inField || e.ctrlKey || e.metaKey || e.altKey) return;
      // Espace sur un bouton : le bouton s'en charge déjà.
      if (e.key === " " && target?.closest("button")) return;
      if (e.key === " ") toggle();
      else if (e.key === ",") step(-1);
      else if (e.key === ".") step(1);
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle, step]);

  const src = source?.path ? videoUrl(source.path) : null;
  const poster = media.hasPreview ? (previewUrl(media.id, 2048) ?? undefined) : undefined;
  const duration = source?.durationMs ?? media.durationMs ?? 0;
  const info = [
    media.width && media.height ? `${media.width} × ${media.height}` : null,
    source?.fps ? t("video.fps", { value: Math.round(source.fps * 100) / 100 }) : null,
    source?.codec ? (CODECS[source.codec] ?? source.codec.toUpperCase()) : null,
    duration ? formatDuration(duration) : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <div className="video-player">
      <div className="video-stage">
        {src && !failed ? (
          <video
            ref={video}
            key={src}
            src={src}
            poster={poster}
            muted={muted}
            playsInline
            preload="metadata"
            onClick={toggle}
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
            onTimeUpdate={(e) => setTime(e.currentTarget.currentTime)}
            onSeeked={(e) => setTime(e.currentTarget.currentTime)}
            onError={() => setFailed(true)}
          />
        ) : (
          <>
            {poster && <img src={poster} alt="" />}
            <p className="video-status" role="status">
              {preparing ? (
                <>
                  <LoaderCircle className="spin" aria-hidden="true" strokeWidth={1.75} />
                  {t("video.preparing")}
                </>
              ) : failed || (source && !source.path) ? (
                t(source?.ffmpeg === false ? "video.noFfmpeg" : "video.unplayable")
              ) : null}
            </p>
          </>
        )}
      </div>
      <div className="video-controls">
        <IconButton
          icon={ChevronLeft}
          label={t("video.previousFrame")}
          shortcut=","
          disabled={!src}
          onClick={() => step(-1)}
        />
        <IconButton
          icon={playing ? Pause : Play}
          label={t(playing ? "video.pause" : "video.play")}
          shortcut={t("video.space")}
          disabled={!src}
          onClick={toggle}
        />
        <IconButton
          icon={ChevronRight}
          label={t("video.nextFrame")}
          shortcut="."
          disabled={!src}
          onClick={() => step(1)}
        />
        <input
          type="range"
          className="video-seek"
          aria-label={t("video.position")}
          min={0}
          max={duration / 1000 || 0}
          step={1 / fps}
          value={time}
          disabled={!src}
          onChange={(e) => {
            const v = video.current;
            if (v) v.currentTime = Number(e.currentTarget.value);
          }}
        />
        <span className="video-time">
          {formatDuration(time * 1000)} / {formatDuration(duration)}
        </span>
        <IconButton
          icon={muted ? VolumeX : Volume2}
          label={t(muted ? "video.unmute" : "video.mute")}
          pressed={muted}
          disabled={!src}
          onClick={() => setMuted(!muted)}
        />
      </div>
      {info && <p className="video-info">{info}</p>}
    </div>
  );
}
