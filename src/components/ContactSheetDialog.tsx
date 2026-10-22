import { save as chooseTarget } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api/core";
import { sessionLabel, useClients } from "../store/clients";
import { useCollections } from "../store/collections";
import {
  type ContactSheet,
  type Orientation,
  type Paper,
  useContactSheet,
} from "../store/contactSheet";
import { deliveryPhotos, useLibrary } from "../store/library";
import { useStudioLogo } from "../store/studioLogo";
import { albumDates } from "../ui/format";
import { ErrorBanner } from "./ErrorBanner";

const COLUMNS = [2, 3, 4, 5, 6, 7, 8];

/**
 * Livrer > Planche contact… : PDF numéroté d'un album, d'une collection ou
 * de la sélection, pour un client ou pour l'archive (cahier des charges, 4.6).
 */
export function ContactSheetDialog() {
  const { t, i18n } = useTranslation();
  const open = useContactSheet((s) => s.open);
  const options = useContactSheet((s) => s.options);
  const running = useContactSheet((s) => s.running);
  const progress = useContactSheet((s) => s.progress);
  const error = useContactSheet((s) => s.error);
  const logo = useStudioLogo((s) => s.url);
  const sessionTypes = useClients((s) => s.sessionTypes);
  const [photographer, setPhotographer] = useState("");
  const st = useContactSheet.getState();

  useEffect(() => {
    if (!open) return;
    void useStudioLogo.getState().load();
    void useClients.getState().load();
    api
      .identityGet()
      .then((i) => setPhotographer(i.name))
      .catch(() => setPhotographer(""));
    const escape = (e: KeyboardEvent) => e.key === "Escape" && useContactSheet.getState().close();
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, [open]);

  if (!open) return null;
  const photos = deliveryPhotos();
  const { albums, albumId, collectionId, selectedIds } = useLibrary.getState();
  const album = albums.find((a) => a.id === albumId);
  const collection = useCollections.getState().collections.find((c) => c.id === collectionId);
  const title = album?.name ?? collection?.name ?? t("contactSheet.allPhotos");
  const sessionType = sessionTypes.find((s) => s.id === album?.sessionTypeId);
  // En-tête du cahier des charges : client et séance, date, photographe.
  const subtitle = [
    album?.clientName ?? null,
    sessionLabel(t, sessionType) || null,
    album ? albumDates(album, i18n.language) : null,
    photographer || null,
    t("contactSheet.count", { count: photos.length }),
    selectedIds.length > 1 ? t("contactSheet.selection") : null,
  ]
    .filter(Boolean)
    .join(" · ");
  const wm = options.watermark;

  const submit = async () => {
    const path = await chooseTarget({
      title: t("contactSheet.title"),
      defaultPath: `${title}.pdf`,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!path) return;
    const sheet: ContactSheet = {
      ...options,
      headerLogo: options.headerLogo && logo !== null,
      title,
      subtitle,
      footer: t("contactSheet.footer", { page: "{page}", pages: "{pages}" }),
      items: photos.map((m) => ({
        mediaId: m.id,
        caption: m.version ? `${m.fileName} · ${m.version}` : m.fileName,
      })),
    };
    await st.run(path, sheet);
  };

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && st.close()}>
      <form
        className="copy-dialog contact-sheet-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="contact-sheet-title"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <h2 id="contact-sheet-title" className="copy-title">
          {t("contactSheet.title")}
        </h2>
        <p className="panel-note">
          {title} · {t("contactSheet.count", { count: photos.length })}
        </p>
        <ErrorBanner error={error} onDismiss={() => useContactSheet.setState({ error: null })} />
        <label className="field-row">
          <span className="field-row-label">{t("contactSheet.columns")}</span>
          <select
            value={options.columns}
            onChange={(e) => st.setOptions({ columns: Number(e.currentTarget.value) })}
          >
            {COLUMNS.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
        </label>
        <label className="field-row">
          <span className="field-row-label">{t("contactSheet.paper")}</span>
          <select
            value={options.paper}
            onChange={(e) => st.setOptions({ paper: e.currentTarget.value as Paper })}
          >
            <option value="a4">A4</option>
            <option value="letter">{t("contactSheet.letter")}</option>
          </select>
        </label>
        <label className="field-row">
          <span className="field-row-label">{t("contactSheet.orientation")}</span>
          <select
            value={options.orientation}
            onChange={(e) => st.setOptions({ orientation: e.currentTarget.value as Orientation })}
          >
            <option value="portrait">{t("contactSheet.portrait")}</option>
            <option value="landscape">{t("contactSheet.landscape")}</option>
          </select>
        </label>
        <label className="check-row">
          <input
            type="checkbox"
            checked={options.headerLogo && logo !== null}
            disabled={logo === null}
            onChange={(e) => st.setOptions({ headerLogo: e.currentTarget.checked })}
          />
          {t("contactSheet.headerLogo")}
        </label>
        <label className="field-row">
          <span className="field-row-label">{t("contactSheet.watermark")}</span>
          <select
            value={wm.kind}
            onChange={(e) => {
              const kind = e.currentTarget.value;
              st.setOptions({
                watermark:
                  kind === "text"
                    ? { kind: "text", text: photographer ? `© ${photographer}` : "©" }
                    : kind === "logo"
                      ? { kind: "logo" }
                      : { kind: "none" },
              });
            }}
          >
            <option value="none">{t("contactSheet.watermarkNone")}</option>
            <option value="text">{t("contactSheet.watermarkText")}</option>
            <option value="logo" disabled={logo === null}>
              {t("deliver.watermarkKind.studioLogo")}
            </option>
          </select>
        </label>
        {wm.kind === "text" && (
          <label className="field-row">
            <span className="field-row-label">{t("deliver.watermarkText")}</span>
            <input
              type="text"
              maxLength={60}
              value={wm.text}
              onChange={(e) =>
                st.setOptions({ watermark: { kind: "text", text: e.currentTarget.value } })
              }
            />
          </label>
        )}
        {logo === null && <p className="panel-note">{t("deliver.noStudioLogo")}</p>}
        <div className="develop-crop-actions">
          {running && progress && (
            <span className="panel-note">
              {t("contactSheet.progress", { done: progress.done, total: progress.total })}
            </span>
          )}
          <span className="toolbar-spacer" />
          <button
            type="button"
            className="button button-small"
            disabled={running}
            onClick={st.close}
          >
            {t("trash.cancel")}
          </button>
          <button
            type="submit"
            className="button button-small button-primary"
            disabled={running || photos.length === 0}
          >
            {t("contactSheet.save")}
          </button>
        </div>
      </form>
    </div>
  );
}
