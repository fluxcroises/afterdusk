import { invoke } from "@tauri-apps/api/core";
import {
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  Pause,
  Play,
  Trash2,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { previewUrl, videoUrl } from "../api/previews";
import { snap, useClips } from "../store/clips";
import type { MediaSummary } from "../store/library";
import { formatDuration, formatTimecode } from "../ui/format";
import { ErrorBanner } from "./ErrorBanner";
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
 * Dérushage : extraits du clip (I pour l'entrée, O pour la sortie, Entrée
 * pour ajouter), marqués sur la barre de position, exportés sans
 * réencodage, un par fichier ou réunis. L'entrée affichée est celle de
 * l'export : recalée sur l'image clé qui la précède.
 */
function ClipsPanel({
  inMs,
  outMs,
  onAdd,
  onPlay,
}: {
  inMs: number | null;
  outMs: number | null;
  onAdd: () => void;
  onPlay: (startMs: number, endMs: number) => void;
}) {
  const { t, i18n } = useTranslation();
  const clips = useClips((s) => s.clips);
  const keyframes = useClips((s) => s.keyframes);
  const progress = useClips((s) => s.progress);
  const exported = useClips((s) => s.exported);
  const error = useClips((s) => s.error);
  const st = useClips.getState();
  const tc = (ms: number) => formatTimecode(ms, i18n.language);
  const snapped = inMs === null ? null : snap(keyframes, inMs);
  const ready = snapped !== null && outMs !== null && outMs > snapped;
  return (
    <section className="video-clips" aria-label={t("clips.title")}>
      <ErrorBanner error={error} onDismiss={() => useClips.setState({ error: null })} />
      <div className="video-clips-marks">
        <span>
          {t("clips.in")} <kbd className="kbd">I</kbd>{" "}
          <strong>{snapped === null ? "–" : tc(snapped)}</strong>
          {snapped !== null && inMs !== null && snapped < inMs - 5 && (
            <span className="panel-note"> {t("clips.snapped", { asked: tc(inMs) })}</span>
          )}
        </span>
        <span>
          {t("clips.out")} <kbd className="kbd">O</kbd>{" "}
          <strong>{outMs === null ? "–" : tc(outMs)}</strong>
        </span>
        <button type="button" className="button button-small" disabled={!ready} onClick={onAdd}>
          {t("clips.add")} <kbd className="kbd">{t("clips.enter")}</kbd>
        </button>
      </div>
      {clips.length === 0 ? (
        <p className="panel-note">{t("clips.empty")}</p>
      ) : (
        <ul className="video-clips-list">
          {clips.map((c) => (
            <li key={c.id}>
              <IconButton
                icon={Play}
                label={t("clips.play", { from: tc(c.startMs), to: tc(c.endMs) })}
                onClick={() => onPlay(c.startMs, c.endMs)}
              />
              <span className="video-clips-range">
                {tc(c.startMs)} – {tc(c.endMs)}
              </span>
              <input
                type="text"
                className="video-clips-name"
                aria-label={t("clips.name")}
                placeholder={t("clips.unnamed")}
                defaultValue={c.name}
                onBlur={(e) => {
                  if (e.currentTarget.value.trim() !== c.name) {
                    void st.rename(c.id, e.currentTarget.value);
                  }
                }}
              />
              <IconButton
                icon={Trash2}
                label={t("clips.remove", { from: tc(c.startMs), to: tc(c.endMs) })}
                onClick={() => void st.remove(c.id)}
              />
            </li>
          ))}
        </ul>
      )}
      <div className="video-clips-actions">
        <button
          type="button"
          className="button button-small"
          disabled={clips.length === 0 || progress !== null}
          onClick={() => void st.exportClips(false)}
        >
          {t("clips.export", { count: clips.length })}
        </button>
        <button
          type="button"
          className="button button-small"
          disabled={clips.length < 2 || progress !== null}
          onClick={() => void st.exportClips(true)}
        >
          {t("clips.join")}
        </button>
        <span className="panel-note" role="status">
          {progress
            ? t("clips.progress", { done: progress.done, total: progress.total })
            : exported.length > 0
              ? t("clips.exported", { count: exported.length })
              : t("clips.noReencode")}
        </span>
      </div>
    </section>
  );
}

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
  const [inMs, setIn] = useState<number | null>(null);
  const [outMs, setOut] = useState<number | null>(null);
  /** Lecture d'un extrait : pause à sa fin. */
  const stopAt = useRef<number | null>(null);

  useEffect(() => {
    void useClips.getState().load(media.id);
  }, [media.id]);

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

  const addClip = useCallback(() => {
    if (inMs === null || outMs === null) return;
    void useClips.getState().add(inMs, outMs);
    setIn(null);
    setOut(null);
  }, [inMs, outMs]);
  const playClip = useCallback((startMs: number, endMs: number) => {
    const v = video.current;
    if (!v) return;
    stopAt.current = endMs / 1000;
    v.currentTime = startMs / 1000;
    void v.play();
  }, []);

  // Espace : lecture ; virgule et point : une image en arrière ou en avant ;
  // I et O : entrée et sortie d'un extrait, Entrée : l'ajouter.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target instanceof Element ? e.target : null;
      // Le curseur de position garde les raccourcis ; un vrai champ, non.
      const inField = target?.closest("input:not([type=range]), textarea, select") != null;
      if (inField || e.ctrlKey || e.metaKey || e.altKey) return;
      // Espace sur un bouton : le bouton s'en charge déjà.
      if ((e.key === " " || e.key === "Enter") && target?.closest("button")) return;
      const now = (video.current?.currentTime ?? 0) * 1000;
      if (e.key === " ") toggle();
      else if (e.key === ",") step(-1);
      else if (e.key === ".") step(1);
      else if (e.key === "i" || e.key === "I") setIn(now);
      else if (e.key === "o" || e.key === "O") setOut(now);
      else if (e.key === "Enter") addClip();
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle, step, addClip]);
  const clips = useClips((s) => s.clips);

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
            onTimeUpdate={(e) => {
              const v = e.currentTarget;
              setTime(v.currentTime);
              if (stopAt.current !== null && v.currentTime >= stopAt.current) {
                stopAt.current = null;
                v.pause();
              }
            }}
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
        <div className="video-seek-wrap">
          {/* Extraits et entrée ou sortie en cours, sous la barre de position. */}
          {duration > 0 &&
            [
              ...clips.map((c) => ({
                key: `c${c.id}`,
                from: c.startMs,
                to: c.endMs,
                pending: false,
              })),
              ...(inMs !== null || outMs !== null
                ? [
                    {
                      key: "pending",
                      from: inMs ?? outMs ?? 0,
                      to:
                        outMs !== null && inMs !== null && outMs > inMs
                          ? outMs
                          : (inMs ?? outMs ?? 0),
                      pending: true,
                    },
                  ]
                : []),
            ].map((m) => (
              <span
                key={m.key}
                className="video-clip-mark"
                data-pending={m.pending || undefined}
                style={{
                  left: `${(m.from / duration) * 100}%`,
                  width: `max(2px, ${((m.to - m.from) / duration) * 100}%)`,
                }}
              />
            ))}
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
        </div>
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
      {src && source?.ffmpeg && (
        <ClipsPanel inMs={inMs} outMs={outMs} onAdd={addClip} onPlay={playClip} />
      )}
    </div>
  );
}
