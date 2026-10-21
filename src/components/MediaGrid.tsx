import { BookImage, Flag, FlagOff, Layers, Star } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { type MediaSummary, type SelectMode, daysLeft, useLibrary } from "../store/library";
import { useBursts } from "../store/bursts";
import { purgeSelection } from "../trash";
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
  const trash = useLibrary((s) => s.trash);
  const ref = useRef<HTMLDivElement>(null);
  const item = (label: string, run: () => void) => (
    <button
      key={label}
      type="button"
      className="menu-row"
      role="menuitem"
      onClick={() => {
        onClose();
        run();
      }}
    >
      <span className="menu-label">{label}</span>
    </button>
  );

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
      {trash ? (
        <>
          {item(t("menu.restore"), () => void useLibrary.getState().restore())}
          {item(t("menu.purge"), () => void purgeSelection())}
        </>
      ) : (
        <>
          {item(t("menu.setAlbumCover"), () => void setAsCover(media.id))}
          {media.companionOf !== null &&
            item(t("menu.stackTop"), () => void useLibrary.getState().setStackTop(media.id))}
          {media.burstId && media.companionOf === null && (
            <>
              {item(t("menu.burstTop"), () => void useLibrary.getState().setStackTop(media.id))}
              {item(t("menu.unstackBurst"), () => void useBursts.getState().unstackSelection())}
            </>
          )}
          <div className="menu-separator" role="separator" />
          {!media.copyOf &&
            item(t("menu.moveToTrash"), () => void useLibrary.getState().moveToTrash())}
          {(media.stackSize > 1 || media.companionOf !== null) && (
            <>
              {item(t("menu.trashJpeg"), () => void useLibrary.getState().trashFormat("image"))}
              {item(t("menu.trashRaw"), () => void useLibrary.getState().trashFormat("raw"))}
            </>
          )}
        </>
      )}
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
    // Seulement une couverture choisie : la couverture par défaut n'a pas de badge.
    s.albums.some((a) => a.id === media.albumId && a.coverChosen && a.coverMediaId === media.id),
  );
  const trashed = useLibrary((s) => s.trash);
  const retentionDays = useLibrary((s) => s.retentionDays);
  const expanded = useLibrary((s) => s.expanded.includes(media.id));
  // Rafale dépliée (depuis n'importe laquelle de ses photos).
  const burstOpen = useLibrary(
    (s) =>
      !!media.burstId &&
      s.expanded.some((e) => s.media.find((x) => x.id === e)?.burstId === media.burstId),
  );
  const burstSize = media.burstSize ?? 1;
  const { t } = useTranslation();
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
      data-below={media.companionOf !== null || undefined}
      data-open={(media.stackSize > 1 && expanded) || undefined}
      data-burst={(burstOpen && burstSize > 1) || undefined}
      title={media.version ? `${media.fileName} · ${media.version}` : media.fileName}
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
        {media.copyOf && (
          <span className="copy-badge" title={media.version ?? undefined}>
            {t("grid.copy")}
          </span>
        )}
        {burstSize > 1 && media.companionOf === null && (
          <span
            className="burst-badge"
            data-open={burstOpen || undefined}
            title={t(burstOpen ? "burst.collapse" : "burst.expand", { count: burstSize }) + " (S)"}
            onClick={(e) => {
              e.stopPropagation();
              void useLibrary.getState().toggleStack(media.id);
            }}
          >
            <Layers aria-hidden="true" strokeWidth={2} />
            {burstSize}
          </span>
        )}
        {media.stackSize > 1 && (
          <span
            className="stack-badge"
            title={t(expanded ? "pile.collapse" : "pile.expand") + " (S)"}
            onClick={(e) => {
              e.stopPropagation();
              void useLibrary.getState().toggleStack(media.id);
            }}
          >
            {media.stackSize}
          </span>
        )}
      </span>
      <span className="media-cell-name">{media.fileName}</span>
      {trashed ? (
        <span className="media-cell-marks media-cell-days">
          {t("trash.daysLeft", { count: daysLeft(media.trashedAt, retentionDays) })}
        </span>
      ) : (
        <MarkBadges media={media} isCover={isCover} />
      )}
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
      title={media.version ? `${media.fileName} · ${media.version}` : media.fileName}
      data-below={media.companionOf !== null || undefined}
      onClick={(e) => void select(media.id, selectMode(e))}
    >
      <Thumbnail src={media.hasPreview ? previewUrl(media.id, 256, version) : null} />
      {media.copyOf && <span className="copy-badge copy-badge-small" aria-hidden="true" />}
      {media.stackSize > 1 && <span className="stack-badge">{media.stackSize}</span>}
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
