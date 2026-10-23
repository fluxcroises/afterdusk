import { VideoPlayer } from "./VideoPlayer";
import { useCallback, useEffect } from "react";
import { useLibrary } from "../store/library";
import { recordTransition } from "../ui/perf";
import { displaySrc, preload } from "../ui/preload";
import { openPhotoMenu } from "../contextMenu";
import { PhotoPane } from "./PhotoPane";

/** Visionneuse : une photo à la fois, voisines décodées d'avance. */
export function Loupe() {
  const media = useLibrary((s) => s.media);
  const selectedId = useLibrary((s) => s.selectedId);
  const versions = useLibrary((s) => s.versions);
  const zoom = useLibrary((s) => s.zoom);
  const setZoom = useLibrary((s) => s.setZoom);
  const index = media.findIndex((m) => m.id === selectedId);
  const current = index >= 0 ? media[index] : undefined;

  // Les voisines sont décodées d'avance, la suivante d'abord.
  useEffect(() => {
    for (const d of [1, -1, 2, -2]) {
      const m = media[index + d];
      const next = m && displaySrc(m, versions[m.id] ?? 0);
      if (next) void preload(next);
    }
  }, [index, media, versions]);

  // Deux images d'animation : la nouvelle photo a été présentée au moins une
  // fois. La mesure majore donc le temps réel, ce qui convient à un plafond.
  const measure = useCallback(() => {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => recordTransition(useLibrary.getState().navigatedAt)),
    );
  }, []);

  if (!current) return null;
  if (current.kind === "video") return <VideoPlayer key={current.id} media={current} />;
  return (
    <PhotoPane
      media={current}
      version={versions[current.id] ?? 0}
      zoom={zoom}
      onZoom={setZoom}
      onShown={measure}
      onMenu={() => void openPhotoMenu(current)}
    />
  );
}
