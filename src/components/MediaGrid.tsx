import { BookImage, Flag, FlagOff, Star } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { type MediaSummary, type SelectMode, useLibrary } from "../store/library";
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

/** Clic simple, Ctrl+clic (ajouter, retirer) ou Maj+clic (plage), comme partout. */
function selectMode(e: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }): SelectMode {
  if (e.shiftKey) return "range";
  return e.ctrlKey || e.metaKey ? "toggle" : "replace";
}

function MediaCell({
  media,
  selected,
  active,
}: {
  media: MediaSummary;
  selected: boolean;
  active: boolean;
}) {
  const version = useLibrary((s) => s.versions[media.id] ?? 0);
  const select = useLibrary((s) => s.select);
  const isCover = useLibrary((s) =>
    s.albums.some((a) => a.id === media.albumId && a.coverMediaId === media.id),
  );
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const ref = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ block: "nearest" });
  }, [active]);

  return (
    <button
      ref={ref}
      type="button"
      className="media-cell"
      aria-pressed={selected}
      data-active={active || undefined}
      data-rejected={media.flag === -1 || undefined}
      title={media.fileName}
      onClick={(e) => void select(media.id, selectMode(e))}
      onDoubleClick={() => useLibrary.getState().setView("loupe")}
      onContextMenu={(e) => {
        e.preventDefault();
        if (!selected) void select(media.id);
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
  const selectedIds = useLibrary((s) => s.selectedIds);
  const chosen = new Set(selectedIds);
  return (
    <div className="media-grid">
      {media.map((m) => (
        <MediaCell key={m.id} media={m} selected={chosen.has(m.id)} active={m.id === selectedId} />
      ))}
    </div>
  );
}

function StripCell({
  media,
  selected,
  active,
}: {
  media: MediaSummary;
  selected: boolean;
  active: boolean;
}) {
  const version = useLibrary((s) => s.versions[media.id] ?? 0);
  const select = useLibrary((s) => s.select);
  const ref = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (active) ref.current?.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [active]);

  return (
    <button
      ref={ref}
      type="button"
      className="strip-cell"
      aria-pressed={selected}
      data-active={active || undefined}
      title={media.fileName}
      onClick={(e) => void select(media.id, selectMode(e))}
    >
      <Thumbnail src={media.hasPreview ? previewUrl(media.id, 256, version) : null} />
    </button>
  );
}

/** Miniatures de la pellicule. */
export function FilmstripThumbs() {
  const media = useLibrary((s) => s.media);
  const selectedId = useLibrary((s) => s.selectedId);
  const selectedIds = useLibrary((s) => s.selectedIds);
  const chosen = new Set(selectedIds);
  return (
    <div className="strip">
      {media.map((m) => (
        <StripCell key={m.id} media={m} selected={chosen.has(m.id)} active={m.id === selectedId} />
      ))}
    </div>
  );
}
