import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

/** Glyphes fins, dessinés comme ceux de Windows 11. */
const GLYPHS = {
  minimize: "M2 6h8",
  maximize: "M2.5 2.5h7v7h-7z",
  restore: "M4 2.5h5.5V8M2.5 4h5.5v5.5H2.5z",
  close: "M2.5 2.5l7 7M9.5 2.5l-7 7",
};

function Glyph({ d }: { d: string }) {
  return (
    <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
      <path d={d} fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}

/** Boutons Réduire, Agrandir et Fermer d'une fenêtre sans cadre (Windows). */
export function WindowControls({ maximizable = true }: { maximizable?: boolean }) {
  const { t } = useTranslation();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    const win = getCurrentWindow();
    const update = () => void win.isMaximized().then(setMaximized);
    update();
    const unlisten = win.onResized(update);
    return () => void unlisten.then((u) => u());
  }, []);

  if (!isTauri()) return null;
  const win = getCurrentWindow();

  return (
    <div className="window-controls">
      <button
        type="button"
        className="window-control"
        aria-label={t("window.minimize")}
        title={t("window.minimize")}
        onClick={() => void win.minimize()}
      >
        <Glyph d={GLYPHS.minimize} />
      </button>
      {maximizable && (
        <button
          type="button"
          className="window-control"
          aria-label={t(maximized ? "window.restore" : "window.maximize")}
          title={t(maximized ? "window.restore" : "window.maximize")}
          onClick={() => void win.toggleMaximize()}
        >
          <Glyph d={maximized ? GLYPHS.restore : GLYPHS.maximize} />
        </button>
      )}
      <button
        type="button"
        className="window-control window-control-close"
        aria-label={t("window.close")}
        title={t("window.close")}
        onClick={() => void win.close()}
      >
        <Glyph d={GLYPHS.close} />
      </button>
    </div>
  );
}
