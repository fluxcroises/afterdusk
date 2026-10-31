import { Camera, MemoryStick, TriangleAlert, Unplug } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useImport } from "../store/import";
import { useLibrary } from "../store/library";
import { useNavigation } from "../store/navigation";
import { LOW_REMAINING, SHOW_FOR, type ShowFor, modelOf, useTether } from "../store/tether";
import { ErrorBanner } from "./ErrorBanner";

/** Ouvre l'album de la séance dans Trier. */
async function showSessionAlbum(albumName: string): Promise<void> {
  useNavigation.getState().goTo("cull");
  const library = useLibrary.getState();
  await library.reloadAlbums();
  const album = useLibrary.getState().albums.find((a) => a.name === albumName);
  if (album) await library.showAlbum(album.id);
}

/**
 * Connexion au boîtier : « X-H2 relié · Démarrer la séance », puis l'état de
 * la séance (photos reçues, place sur la carte, débranchement).
 */
export function TetherBanner() {
  const { t } = useTranslation();
  const status = useTether((s) => s.status);
  const lastCamera = useTether((s) => s.lastCamera);
  const dismissed = useTether((s) => s.dismissed);
  const seenLeft = useTether((s) => s.seenLeft);
  const seenFailed = useTether((s) => s.seenFailed);
  const starting = useTether((s) => s.starting);
  const st = useTether.getState();
  const camera = status.camera ?? lastCamera;
  const model = camera ? modelOf(camera) : "";

  let banner = null;
  if (!status.active) {
    if (status.camera && dismissed !== status.camera) {
      banner = (
        <div className="banner banner-info device-banner" role="status">
          <Camera aria-hidden="true" strokeWidth={1.75} />
          <span>{t("tether.detected", { camera: status.camera })}</span>
          <button
            type="button"
            className="button button-small button-primary"
            onClick={st.openStart}
          >
            {t("tether.start")}
          </button>
          <button type="button" className="button button-small" onClick={st.dismiss}>
            {t("import.dismiss")}
          </button>
        </div>
      );
    }
  } else if (status.connected && status.failed.length > seenFailed) {
    const names = status.failed.slice(seenFailed);
    banner = (
      <div className="banner banner-warning device-banner" role="alert">
        <TriangleAlert aria-hidden="true" strokeWidth={1.75} />
        <span>{t("tether.failed", { count: names.length, names: names.join(", ") })}</span>
        <button type="button" className="button button-small" onClick={st.acknowledge}>
          {t("tether.understood")}
        </button>
      </div>
    );
  } else if (status.connected && status.leftOnCard > seenLeft) {
    banner = (
      <div className="banner banner-warning device-banner" role="alert">
        <MemoryStick aria-hidden="true" strokeWidth={1.75} />
        <span>{t("tether.leftOnCard", { model, count: status.leftOnCard - seenLeft })}</span>
        <button type="button" className="button button-small" onClick={st.acknowledge}>
          {t("tether.understood")}
        </button>
      </div>
    );
  } else {
    const low = status.connected && status.remaining !== null && status.remaining < LOW_REMAINING;
    const parts = status.connected
      ? [
          model,
          t("tether.album", { album: status.albumName }),
          t("tether.received", { count: status.received }),
          status.remaining !== null &&
            t(low ? "tether.lowRemaining" : "tether.remaining", { count: status.remaining }),
        ]
      : [
          camera && !status.camera
            ? t("tether.unplugged", { model })
            : t("tether.connecting", { model: model || t("tether.camera") }),
          t("tether.received", { count: status.received }),
        ];
    banner = (
      <div
        className={`banner ${low ? "banner-warning" : "banner-info"} device-banner tether-banner`}
        role="status"
      >
        {low ? (
          <TriangleAlert aria-hidden="true" strokeWidth={1.75} />
        ) : status.connected ? (
          <span className="tether-live" aria-hidden="true" />
        ) : (
          <Unplug aria-hidden="true" strokeWidth={1.75} />
        )}
        <span>
          {parts.filter(Boolean).join(" · ")}
          {camera && !status.camera && (
            <span className="tether-hint"> {t("tether.unpluggedHint")}</span>
          )}
        </span>
        {status.connected && (
          <button
            type="button"
            className="button button-small"
            onClick={() => void showSessionAlbum(status.albumName)}
          >
            {t("tether.showAlbum")}
          </button>
        )}
        <button type="button" className="button button-small" onClick={() => void st.stop()}>
          {t("tether.stop")}
        </button>
      </div>
    );
  }

  return (
    <>
      {banner}
      {starting && camera && <StartDialog model={modelOf(camera)} />}
    </>
  );
}

/** Fenêtre « Séance reliée au X-H2 » : album, sauvegarde, affichage en grand. */
function StartDialog({ model }: { model: string }) {
  const { t, i18n } = useTranslation();
  const libraries = useImport((s) => s.libraries);
  const libraryId = useImport((s) => s.libraryId);
  const library = libraries.find((l) => l.id === libraryId) ?? null;
  const showFor = useTether((s) => s.showFor);
  const error = useTether((s) => s.error);
  const st = useTether.getState();
  const [album, setAlbum] = useState(() =>
    t("tether.defaultAlbum", {
      model,
      date: new Date().toLocaleDateString(i18n.language),
    }),
  );
  const [backup, setBackup] = useState(true);

  useEffect(() => {
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") useTether.getState().closeStart();
    };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, []);

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && st.closeStart()}
    >
      <form
        className="copy-dialog tether-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="tether-title"
        onSubmit={(e) => {
          e.preventDefault();
          if (library) void st.start(library.id, album, backup && !!library.backupPath);
        }}
      >
        <h2 id="tether-title" className="copy-title">
          {t("tether.title", { model })}
        </h2>
        <ErrorBanner error={error} onDismiss={() => useTether.setState({ error: null })} />
        <label className="field-row">
          <span className="field-row-label">{t("tether.albumLabel")}</span>
          <input
            type="text"
            value={album}
            maxLength={120}
            required
            autoFocus
            onChange={(e) => setAlbum(e.currentTarget.value)}
          />
        </label>
        <label className="field-row">
          <span className="field-row-label">{t("tether.showFor")}</span>
          <select
            value={showFor}
            onChange={(e) => st.setShowFor(Number(e.currentTarget.value) as ShowFor)}
          >
            {SHOW_FOR.map((s) => (
              <option key={s} value={s}>
                {s === 0
                  ? t("tether.untilNext")
                  : s === -1
                    ? t("tether.never")
                    : t("tether.seconds", { count: s })}
              </option>
            ))}
          </select>
        </label>
        {library?.backupPath && (
          <label className="check-row">
            <input
              type="checkbox"
              checked={backup}
              onChange={(e) => setBackup(e.currentTarget.checked)}
            />
            {t("tether.backup")}
          </label>
        )}
        <p className="panel-note">
          {library ? t("tether.note", { library: library.name }) : t("tether.noLibrary")}
        </p>
        <p className="panel-note">{t("tether.profile")}</p>
        <div className="develop-crop-actions">
          <span className="toolbar-spacer" />
          <button type="button" className="button button-small" onClick={st.closeStart}>
            {t("develop.crop.cancel")}
          </button>
          <button
            type="submit"
            className="button button-small button-primary"
            disabled={!library || album.trim() === ""}
          >
            {t("tether.confirm")}
          </button>
        </div>
      </form>
    </div>
  );
}
