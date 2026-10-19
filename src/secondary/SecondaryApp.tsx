import { isTauri } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { Columns2, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../components/IconButton";
import { PhotoPane, type Zoom } from "../components/PhotoPane";
import { displaySrc, preload } from "../ui/preload";
import { toggleSecondary } from "../windows";
import { EVENT_READY, EVENT_STATE, type SecondaryState } from "./sync";
import "../App.css";

type Mode = "loupe" | "compare";
const MODE_KEY = "afterdusk.secondaryMode";

function readMode(): Mode {
  try {
    return window.localStorage.getItem(MODE_KEY) === "compare" ? "compare" : "loupe";
  } catch {
    return "loupe";
  }
}

/** Affichage du second écran, piloté par l'état reçu de la fenêtre principale. */
export function SecondaryView({ state }: { state: SecondaryState | null }) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<Mode>(readMode);
  const [zoom, setZoom] = useState<Zoom>(null);

  const choose = (m: Mode) => {
    setMode(m);
    setZoom(null);
    try {
      window.localStorage.setItem(MODE_KEY, m);
    } catch {
      // Préférence de confort.
    }
  };

  // Les voisines de la photo active sont décodées d'avance.
  useEffect(() => {
    for (const m of state?.photos ?? []) {
      const src = displaySrc(m, state?.versions[m.id] ?? 0);
      if (src) void preload(src);
    }
  }, [state]);

  const byId = new Map((state?.photos ?? []).map((m) => [m.id, m] as const));
  const active = state?.activeId != null ? byId.get(state.activeId) : undefined;
  const panes =
    mode === "compare"
      ? (state?.compare ?? []).map((id) => byId.get(id)).filter((m) => m !== undefined)
      : active
        ? [active]
        : [];

  return (
    <div className="secondary">
      <div className="secondary-bar" data-tauri-drag-region>
        <span className="welcome-brand" data-tauri-drag-region>
          <span className="logo" aria-hidden="true" />
          <span className="brand-name">{t("secondary.title")}</span>
        </span>
        <span className="toolbar-spacer" data-tauri-drag-region />
        <IconButton
          icon={Square}
          label={t("toolbar.loupe")}
          pressed={mode === "loupe"}
          onClick={() => choose("loupe")}
        />
        <IconButton
          icon={Columns2}
          label={t("toolbar.compare")}
          pressed={mode === "compare"}
          onClick={() => choose("compare")}
        />
        <IconButton
          icon={X}
          label={t("secondary.close")}
          shortcut={t("keys.escape")}
          onClick={() => void toggleSecondary()}
        />
      </div>
      {panes.length === 0 ? (
        <p className="secondary-empty">{t("secondary.empty")}</p>
      ) : (
        <div className="compare" data-count={panes.length}>
          {panes.map((m) => (
            <figure
              key={m.id}
              className="compare-pane"
              data-active={(mode === "compare" && m.id === state?.activeId) || undefined}
            >
              <PhotoPane
                media={m}
                version={state?.versions[m.id] ?? 0}
                zoom={zoom}
                onZoom={setZoom}
              />
            </figure>
          ))}
        </div>
      )}
    </div>
  );
}

/** Fenêtre du second écran. */
export function SecondaryApp() {
  const [state, setState] = useState<SecondaryState | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen<SecondaryState>(EVENT_STATE, (e) => setState(e.payload));
    // Demande l'état courant à la fenêtre principale.
    void unlisten.then(() => emitTo("main", EVENT_READY));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void toggleSecondary();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      void unlisten.then((u) => u());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  return <SecondaryView state={state} />;
}
