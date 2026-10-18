import { useEffect, useRef } from "react";
import { previewUrl } from "../api/previews";
import { type MediaSummary, useLibrary } from "../store/library";
import { Thumbnail } from "./Thumbnail";

function MediaCell({ media, selected }: { media: MediaSummary; selected: boolean }) {
  const version = useLibrary((s) => s.versions[media.id] ?? 0);
  const select = useLibrary((s) => s.select);
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
      title={media.fileName}
      onClick={() => void select(media.id)}
    >
      <span className="media-cell-image">
        <Thumbnail
          src={media.hasPreview ? previewUrl(media.id, 256, version) : null}
          fallback={media.fileName.split(".").pop()?.toUpperCase()}
        />
      </span>
      <span className="media-cell-name">{media.fileName}</span>
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
