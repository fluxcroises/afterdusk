import { BookImage, Flag, FlagOff, Star } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { type MediaSummary, useLibrary } from "../store/library";
import { Thumbnail } from "./Thumbnail";

/** Menu contextuel d'une miniature (clic droit). */
function MediaContextMenu({
  media,
  at,
  onClose,
}: {
  media: MediaSummary;
  at: { x: number; y: number };
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const setAsCover = useLibrary((s) => s.setAsCover);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ref.current?.querySelector("button")?.focus();
    const close = (e: Event) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const escape = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", escape);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", escape);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  return createPortal(
    <div
      ref={ref}
      className="menu-list context-menu"
      role="menu"
      style={{ left: at.x, top: at.y }}
      onClick={(e) => e.stopPropagation()}
    >
      <button
        type="button"
        className="menu-row"
        role="menuitem"
        onClick={() => {
          onClose();
          void setAsCover(media.id);
        }}
      >
        <span className="menu-label">{t("menu.setAlbumCover")}</span>
      </button>
    </div>,
    document.body,
  );
}

/** Note, drapeau et couleur sous la miniature, comme dans Lightroom. */
function MarkBadges({ media, isCover }: { media: MediaSummary; isCover: boolean }) {
  const { t } = useTranslation();
  return (
    <span className="media-cell-marks">
      <span
        className="media-cell-stars"
        role="img"
        aria-label={t("toolbar.stars", { count: media.rating })}
      >
        {[1, 2, 3, 4, 5].map((n) => (
          <Star
            key={n}
            aria-hidden="true"
            strokeWidth={1.5}
            fill={n <= media.rating ? "currentColor" : "none"}
            data-on={n <= media.rating || undefined}
          />
        ))}
      </span>
      <span className="toolbar-spacer" />
      {isCover && (
        <BookImage className="media-cell-flag" role="img" aria-label={t("grid.albumCover")} />
      )}
      {media.flag === 1 && (
        <Flag
          className="media-cell-flag"
          role="img"
          aria-label={t("toolbar.pick")}
          fill="currentColor"
        />
      )}
      {media.flag === -1 && (
        <FlagOff className="media-cell-flag" role="img" aria-label={t("toolbar.reject")} />
      )}
      {media.color && (
        <span
          className={"color-chip color-" + media.color}
          role="img"
          aria-label={t("toolbar.colors." + media.color)}
        />
      )}
    </span>
  );
}

function MediaCell({ media, selected }: { media: MediaSummary; selected: boolean }) {
  const version = useLibrary((s) => s.versions[media.id] ?? 0);
  const select = useLibrary((s) => s.select);
  const isCover = useLibrary((s) =>
    s.albums.some((a) => a.id === media.albumId && a.coverMediaId === media.id),
  );
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const ref = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (selected) ref.current?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  return (
    <button
      ref={ref}
      type="button"
      className="media-cell"
      aria-pressed={selected}
      data-rejected={media.flag === -1 || undefined}
      title={media.fileName}
      onClick={() => void select(media.id)}
      onDoubleClick={() => useLibrary.getState().setView("loupe")}
      onContextMenu={(e) => {
        e.preventDefault();
        void select(media.id);
        setMenu({ x: e.clientX, y: e.clientY });
      }}
    >
      <span className="media-cell-image">
        <Thumbnail
          src={media.hasPreview ? previewUrl(media.id, 256, version) : null}
          fallback={media.fileName.split(".").pop()?.toUpperCase()}
        />
      </span>
      <span className="media-cell-name">{media.fileName}</span>
      <MarkBadges media={media} isCover={isCover} />
      {menu && <MediaContextMenu media={media} at={menu} onClose={() => setMenu(null)} />}
    </button>
  );
}

/** Grille des photos de la bibliothèque ou de l'album affiché. */
export function MediaGrid() {
  const media = useLibrary((s) => s.media);
  const selectedId = useLibrary((s) => s.selectedId);
  return (
    <div className="media-grid">
      {media.map((m) => (
        <MediaCell key={m.id} media={m} selected={m.id === selectedId} />
      ))}
    </div>
  );
}

function StripCell({ media, selected }: { media: MediaSummary; selected: boolean }) {
  const version = useLibrary((s) => s.versions[media.id] ?? 0);
  const select = useLibrary((s) => s.select);
  const ref = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (selected) ref.current?.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [selected]);

  return (
    <button
      ref={ref}
      type="button"
      className="strip-cell"
      aria-pressed={selected}
      title={media.fileName}
      onClick={() => void select(media.id)}
    >
      <Thumbnail src={media.hasPreview ? previewUrl(media.id, 256, version) : null} />
    </button>
  );
}

/** Miniatures de la pellicule. */
export function FilmstripThumbs() {
  const media = useLibrary((s) => s.media);
  const selectedId = useLibrary((s) => s.selectedId);
  return (
    <div className="strip">
      {media.map((m) => (
        <StripCell key={m.id} media={m} selected={m.id === selectedId} />
      ))}
    </div>
  );
}
