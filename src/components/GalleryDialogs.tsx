import { invoke } from "@tauri-apps/api/core";
import { open as chooseFile } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api/core";
import { sessionLabel, useClients } from "../store/clients";
import { useCollections } from "../store/collections";
import { type GalleryTheme, useGallery } from "../store/gallery";
import { deliveryPhotos, useLibrary } from "../store/library";
import { useStudioLogo } from "../store/studioLogo";
import { albumDates } from "../ui/format";
import { ErrorBanner } from "./ErrorBanner";

/** Textes de la page générée, dans la langue de l'interface. */
const PAGE_TEXTS = [
  "photo",
  "pick",
  "unpick",
  "send",
  "sendHint",
  "sendHintFile",
  "mail",
  "file",
  "copy",
  "copied",
  "close",
  "previous",
  "next",
  "download",
  "emptySelection",
  "selectionOne",
  "selectionOther",
  "listIntro",
  "mailSubject",
  "footer",
] as const;

function useEscape(active: boolean, close: () => void) {
  useEffect(() => {
    if (!active) return;
    const escape = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, [active, close]);
}

/** Livrer > Galerie web… : un site statique à déposer chez soi. */
export function GalleryDialog() {
  const { t, i18n } = useTranslation();
  const open = useGallery((s) => s.open);
  const options = useGallery((s) => s.options);
  const running = useGallery((s) => s.running);
  const progress = useGallery((s) => s.progress);
  const result = useGallery((s) => s.result);
  const error = useGallery((s) => s.error);
  const logo = useStudioLogo((s) => s.url);
  const sessionTypes = useClients((s) => s.sessionTypes);
  const [email, setEmail] = useState("");
  const [photographer, setPhotographer] = useState("");
  const [title, setTitle] = useState<string | null>(null);
  const [intro, setIntro] = useState("");
  const st = useGallery.getState();
  useEscape(open, st.close);

  useEffect(() => {
    if (!open) return;
    void useStudioLogo.getState().load();
    void useClients.getState().load();
    api
      .identityGet()
      .then((i) => {
        setEmail(i.email ?? "");
        setPhotographer(i.name);
      })
      .catch(() => setEmail(""));
  }, [open]);

  if (!open) return null;
  const photos = deliveryPhotos();
  const { albums, albumId, collectionId, libraryId } = useLibrary.getState();
  const album = albums.find((a) => a.id === albumId);
  const collection = useCollections.getState().collections.find((c) => c.id === collectionId);
  const defaultTitle = album?.name ?? collection?.name ?? t("contactSheet.allPhotos");
  const shownTitle = title ?? defaultTitle;
  const subtitle = [
    album?.clientName ?? null,
    sessionLabel(
      t,
      sessionTypes.find((s) => s.id === album?.sessionTypeId),
    ) || null,
    album ? albumDates(album, i18n.language) : null,
    photographer || null,
  ]
    .filter(Boolean)
    .join(" · ");
  const wm = options.watermark;

  const submit = async () => {
    if (libraryId === null || !shownTitle.trim()) return;
    const destination = await chooseFile({ directory: true, title: t("gallery.chooseFolder") });
    if (typeof destination !== "string") return;
    const texts: Record<string, string> = { lang: i18n.language };
    for (const key of PAGE_TEXTS) texts[key] = t(`gallery.page.${key}`);
    await st.run({
      ...options,
      libraryId,
      title: shownTitle.trim(),
      subtitle,
      intro,
      albumId: album?.id ?? null,
      clientId: album?.clientId ?? null,
      mediaIds: photos.map((m) => m.id),
      destination,
      texts,
    });
  };

  const close = () => {
    setTitle(null);
    setIntro("");
    st.close();
  };

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <form
        className="copy-dialog gallery-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="gallery-title"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <h2 id="gallery-title" className="copy-title">
          {t("gallery.title")}
        </h2>
        <ErrorBanner error={error} onDismiss={() => useGallery.setState({ error: null })} />
        {result ? (
          <>
            <p>
              {t("gallery.done", { count: result.photos, code: result.code })}
              {result.missing > 0 && ` ${t("gallery.missing", { count: result.missing })}`}
            </p>
            <p className="panel-note">{t("gallery.hosting")}</p>
            <div className="develop-crop-actions">
              <span className="toolbar-spacer" />
              <button
                type="button"
                className="button button-small"
                onClick={() => void invoke("export_reveal", { destination: result.folder })}
              >
                {t("gallery.reveal")}
              </button>
              <button type="button" className="button button-small button-primary" onClick={close}>
                {t("gallery.close")}
              </button>
            </div>
          </>
        ) : (
          <>
            <p className="panel-note">{t("contactSheet.count", { count: photos.length })}</p>
            <label className="field-row">
              <span className="field-row-label">{t("gallery.name")}</span>
              <input
                type="text"
                required
                maxLength={120}
                value={shownTitle}
                onChange={(e) => setTitle(e.currentTarget.value)}
              />
            </label>
            <label className="field-row gallery-intro">
              <span className="field-row-label">{t("gallery.intro")}</span>
              <textarea
                rows={3}
                maxLength={2000}
                value={intro}
                onChange={(e) => setIntro(e.currentTarget.value)}
              />
            </label>
            <label className="field-row">
              <span className="field-row-label">{t("gallery.theme")}</span>
              <select
                value={options.theme}
                onChange={(e) => st.setOptions({ theme: e.currentTarget.value as GalleryTheme })}
              >
                <option value="light">{t("gallery.light")}</option>
                <option value="dark">{t("gallery.dark")}</option>
              </select>
            </label>
            <label className="check-row">
              <input
                type="checkbox"
                checked={options.selection}
                onChange={(e) => st.setOptions({ selection: e.currentTarget.checked })}
              />
              {t("gallery.selection")}
            </label>
            {options.selection && !email && <p className="panel-note">{t("gallery.noEmail")}</p>}
            <label className="check-row">
              <input
                type="checkbox"
                checked={options.allowDownload}
                onChange={(e) => st.setOptions({ allowDownload: e.currentTarget.checked })}
              />
              {t("gallery.allowDownload")}
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
            <div className="develop-crop-actions">
              {running && progress && (
                <span className="panel-note">
                  {t("gallery.progress", { done: progress.done, total: progress.total })}
                </span>
              )}
              <span className="toolbar-spacer" />
              <button
                type="button"
                className="button button-small"
                disabled={running}
                onClick={close}
              >
                {t("trash.cancel")}
              </button>
              <button
                type="submit"
                className="button button-small button-primary"
                disabled={running || photos.length === 0 || !shownTitle.trim()}
              >
                {t("gallery.create")}
              </button>
            </div>
          </>
        )}
      </form>
    </div>
  );
}

/** Livrer > Sélection client… : relit le courriel ou le fichier du client. */
export function SelectionImportDialog() {
  const { t } = useTranslation();
  const open = useGallery((s) => s.importOpen);
  const text = useGallery((s) => s.text);
  const found = useGallery((s) => s.found);
  const error = useGallery((s) => s.importError);
  const st = useGallery.getState();
  useEscape(open, st.closeImport);
  if (!open) return null;

  const otherLibrary = found !== null && found.libraryId !== useLibrary.getState().libraryId;

  const openFile = async () => {
    const path = await chooseFile({
      multiple: false,
      title: t("gallery.import.openFile"),
      filters: [{ name: t("gallery.import.textFiles"), extensions: ["txt", "eml"] }],
    });
    if (typeof path === "string") await st.readFile(path);
  };

  const apply = async () => {
    if (!found) return;
    const id = await st.apply(
      t("gallery.import.collectionName", { name: found.clientName ?? found.title }),
    );
    if (id === null || otherLibrary) return;
    const library = useLibrary.getState();
    await useCollections.getState().load();
    await library.showCollection(id);
  };

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && st.closeImport()}
    >
      <div
        className="copy-dialog gallery-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="selection-import-title"
      >
        <h2 id="selection-import-title" className="copy-title">
          {t("gallery.import.title")}
        </h2>
        <p className="panel-note">{t("gallery.import.hint")}</p>
        <textarea
          className="selection-text"
          aria-label={t("gallery.import.paste")}
          placeholder={t("gallery.import.paste")}
          rows={5}
          value={text}
          onChange={(e) => void st.read(e.currentTarget.value)}
        />
        <ErrorBanner error={error} onDismiss={() => useGallery.setState({ importError: null })} />
        {found && (
          <div role="status">
            <p>
              {t("gallery.import.found", {
                title: found.title,
                count: found.mediaIds.length,
              })}
              {found.clientName && ` · ${found.clientName}`}
            </p>
            {found.unknown.length > 0 && (
              <p className="panel-note">
                {t("gallery.import.unknown", {
                  count: found.unknown.length,
                  numbers: found.unknown.join(", "),
                })}
              </p>
            )}
            {otherLibrary && <p className="panel-note">{t("gallery.import.otherLibrary")}</p>}
          </div>
        )}
        <div className="develop-crop-actions">
          <button type="button" className="button button-small" onClick={() => void openFile()}>
            {t("gallery.import.openFile")}
          </button>
          <span className="toolbar-spacer" />
          <button type="button" className="button button-small" onClick={st.closeImport}>
            {t("trash.cancel")}
          </button>
          <button
            type="button"
            className="button button-small button-primary"
            disabled={!found || found.mediaIds.length === 0}
            onClick={() => void apply()}
          >
            {t("gallery.import.apply")}
          </button>
        </div>
      </div>
    </div>
  );
}
