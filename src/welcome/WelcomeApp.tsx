import { AppDialog } from "../components/AppDialog";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useRescan } from "../store/rescan";
import { Check, ChevronDown, FolderOpen, HardDriveDownload, Images } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { newLibrary, pickFolder } from "../actions";
import { api, type RecentImport } from "../api/core";
import { previewUrl } from "../api/previews";
import { useAppEvents } from "../appEvents";
import { DeviceBanner } from "../components/DeviceBanner";
import { OfflineBanner } from "../components/OfflineBanner";
import { Thumbnail } from "../components/Thumbnail";
import { WindowControls } from "../components/WindowControls";
import { drawsOwnMenus } from "../platform";
import { useImport } from "../store/import";
import { Onboarding } from "./Onboarding";
import { useLayout } from "../store/layout";
import { type AlbumSummary, useLibrary } from "../store/library";
import { useReminders } from "../store/maintenance";
import { formatDay } from "../ui/format";
import { greetingKey } from "../ui/greeting";
import { openMain, setWelcomeOnStartup, welcomeOnStartup } from "../windows";
import defaultPhoto from "../assets/accueil-cabourg.jpg";
import "../App.css";

interface Highlight {
  mediaId: number;
  albumName: string;
  capturedAt: string | null;
}

/** Données de l'accueil, rechargées quand la bibliothèque change ou au retour sur l'accueil. */
function pickHighlight(libraryId: number): Promise<Highlight | null> {
  return invoke<Highlight[]>("welcome_highlights", { libraryId }).then(
    // Une photo différente à chaque ouverture, comme les écrans de démarrage d'Adobe.
    (list) => list[Math.floor(Math.random() * list.length)] ?? null,
  );
}

function useWelcomeData() {
  const libraryId = useImport((s) => s.libraryId);
  const libraries = useImport((s) => s.libraries);
  const [recent, setRecent] = useState<RecentImport[]>([]);
  const [highlight, setHighlight] = useState<Highlight | null>(null);
  const previewsReady = useLibrary((s) => Object.keys(s.versions).length);

  useEffect(() => {
    if (!isTauri() || libraryId === null) return;
    let current = true;
    api
      .importsRecent(libraryId)
      .then((r) => current && setRecent(r))
      .catch(() => current && setRecent([]));
    pickHighlight(libraryId)
      .then((h) => current && setHighlight(h))
      .catch(() => current && setHighlight(null));
    return () => {
      current = false;
    };
    // `libraries` change à chaque rafraîchissement de l'accueil.
  }, [libraryId, libraries]);

  // Sans illustration (premier import), un nouvel aperçu prêt relance la recherche.
  const missing = highlight === null;
  useEffect(() => {
    if (!isTauri() || libraryId === null || !missing || previewsReady === 0) return;
    let current = true;
    pickHighlight(libraryId)
      .then((h) => current && h && setHighlight(h))
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [libraryId, missing, previewsReady]);

  return libraryId === null ? { recent: [], highlight: null } : { recent, highlight };
}

async function importFolder(title: string) {
  const path = await pickFolder(title);
  if (path) await openMain({ screen: "import", scanPath: path });
}

function formatDate(value: string | null, language: string): string | null {
  if (!value) return null;
  const date = new Date(value.length === 10 ? `${value}T12:00:00` : value);
  return Number.isNaN(date.getTime())
    ? null
    : new Intl.DateTimeFormat(language, { dateStyle: "long" }).format(date);
}

/** Bibliothèque courante, avec un menu pour en changer ou en créer une. */
function LibrarySwitcher() {
  const { t } = useTranslation();
  const libraries = useImport((s) => s.libraries);
  const libraryId = useImport((s) => s.libraryId);
  const selectLibrary = useImport((s) => s.selectLibrary);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const current = libraries.find((l) => l.id === libraryId);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);

  return (
    <div className="library-switcher" ref={ref}>
      <button
        type="button"
        className="library-switcher-button"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        {current ? current.name : t("app.noLibrary")}
        <ChevronDown aria-hidden="true" strokeWidth={2} />
      </button>
      {open && (
        <div className="menu-list" role="menu">
          {libraries.map((l) => (
            <button
              key={l.id}
              type="button"
              className="menu-row"
              role="menuitemradio"
              aria-checked={l.id === libraryId}
              onClick={() => {
                selectLibrary(l.id);
                setOpen(false);
              }}
            >
              <span className="menu-check">
                {l.id === libraryId && <Check aria-hidden="true" strokeWidth={2} />}
              </span>
              <span className="menu-label">{l.name}</span>
              <span className="menu-accelerator">{l.rootPath}</span>
            </button>
          ))}
          {libraries.length > 0 && <div className="menu-separator" role="separator" />}
          <button
            type="button"
            className="menu-row"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              void newLibrary();
            }}
          >
            <span className="menu-check" />
            <span className="menu-label">{t("import.newLibrary")}</span>
          </button>
        </div>
      )}
    </div>
  );
}

/**
 * En-tête de l'accueil : la photo d'illustration occupe le haut de la fenêtre,
 * la barre de titre passe par-dessus, la salutation et les actions sont posées
 * en bas de l'image.
 */
function Hero({ highlight }: { highlight: Highlight | null }) {
  const { t, i18n } = useTranslation();
  const hasLibrary = useImport((s) => s.libraryId !== null);
  const date = formatDate(highlight?.capturedAt ?? null, i18n.language);
  // Tant que la bibliothèque n'a pas de photo à mettre en avant : Cabourg, par l'auteur.
  const caption = highlight
    ? [highlight.albumName, date].filter(Boolean).join(", ")
    : t("home.defaultPhotoCredit");

  return (
    <header className="welcome-hero" data-tauri-drag-region>
      <img
        key={highlight?.mediaId ?? "default"}
        className="welcome-hero-photo"
        data-default={highlight ? undefined : true}
        src={highlight ? (previewUrl(highlight.mediaId, 2048) ?? defaultPhoto) : defaultPhoto}
        alt=""
        decoding="async"
        data-tauri-drag-region
      />
      <div className="welcome-hero-top" data-tauri-drag-region>
        <span className="welcome-brand" data-tauri-drag-region>
          <span className="logo" aria-hidden="true" />
          <span className="brand-name">{t("app.name")}</span>
        </span>
        <span className="toolbar-spacer" data-tauri-drag-region />
        <span className="welcome-hero-caption" data-tauri-drag-region>
          {caption}
        </span>
        {drawsOwnMenus && <WindowControls />}
      </div>
      <div className="welcome-hero-bottom" data-tauri-drag-region>
        <div className="welcome-identity">
          <h1 className="welcome-greeting">{t(greetingKey())}</h1>
          <LibrarySwitcher />
        </div>
        <span className="toolbar-spacer" data-tauri-drag-region />
        {hasLibrary && (
          <div className="welcome-actions">
            <button
              type="button"
              className="button button-ghost"
              disabled={!hasLibrary}
              onClick={() => void openMain({ screen: "library", albumId: null })}
            >
              <Images aria-hidden="true" strokeWidth={1.75} />
              {t("modules.library")}
            </button>
            <button
              type="button"
              className="button button-ghost"
              disabled={!hasLibrary}
              onClick={() => void importFolder(t("import.chooseFolder"))}
            >
              <FolderOpen aria-hidden="true" strokeWidth={1.75} />
              {t("home.folder")}
            </button>
            <button
              type="button"
              className="button button-primary button-inline"
              disabled={!hasLibrary}
              onClick={() => void openMain({ screen: "import" })}
            >
              <HardDriveDownload aria-hidden="true" strokeWidth={1.75} />
              {t("modules.import")}
            </button>
          </div>
        )}
      </div>
    </header>
  );
}

/** Planche contact du dernier import. */
function ContactSheet({ latest }: { latest: RecentImport }) {
  const { t, i18n } = useTranslation();
  const versions = useLibrary((s) => s.versions);
  const date = formatDate(latest.date, i18n.language);
  return (
    <section className="welcome-section">
      <header className="welcome-section-header">
        <h2>{t("home.lastImportTitle", { source: latest.sourceLabel })}</h2>
        <span className="welcome-section-meta">
          {[date, t("toolbar.photoCount", { count: latest.count })].filter(Boolean).join(" · ")}
        </span>
        <span className="toolbar-spacer" />
        <button
          type="button"
          className="link-button"
          onClick={() => void openMain({ screen: "cull" })}
        >
          {t("home.cullThem")}
        </button>
      </header>
      <div className="contact-sheet">
        {latest.mediaIds.slice(0, 12).map((id) => (
          <button
            key={id}
            type="button"
            className="contact-sheet-cell"
            onClick={() => void openMain({ screen: "cull", mediaId: id })}
          >
            <Thumbnail src={previewUrl(id, 256, versions[id] ?? 0)} />
          </button>
        ))}
      </div>
    </section>
  );
}

function AlbumRow({ album }: { album: AlbumSummary }) {
  const { t, i18n } = useTranslation();
  const versions = useLibrary((s) => s.versions);
  const cover = album.coverMediaId;
  return (
    <button
      type="button"
      className="album-row"
      onClick={() => void openMain({ screen: "cull", albumId: album.id })}
    >
      <span className="album-row-cover">
        <Thumbnail src={cover === null ? null : previewUrl(cover, 256, versions[cover] ?? 0)} />
      </span>
      <span className="album-row-text">
        <span className="album-row-name">{album.name}</span>
        <span className="album-row-meta">{formatDate(album.startDate, i18n.language)}</span>
      </span>
      <span className="album-row-count">{t("toolbar.photoCount", { count: album.count })}</span>
    </button>
  );
}

/** Liste des albums, du plus récent au plus ancien ; dépliable. */
function AlbumList() {
  const { t } = useTranslation();
  const albums = useLibrary((s) => s.albums);
  const [all, setAll] = useState(false);
  const shown = all ? albums : albums.slice(0, 6);
  return (
    <section className="welcome-section">
      <header className="welcome-section-header">
        <h2>{t("catalog.albums")}</h2>
        <span className="toolbar-spacer" />
        {albums.length > 6 && (
          <button type="button" className="link-button" onClick={() => setAll(!all)}>
            {all ? t("home.fewerAlbums") : t("home.allAlbums", { count: albums.length })}
          </button>
        )}
      </header>
      {albums.length === 0 ? (
        <p className="welcome-hint">{t("catalog.noAlbums")}</p>
      ) : (
        <div className="album-list">
          {shown.map((a) => (
            <AlbumRow key={a.id} album={a} />
          ))}
        </div>
      )}
    </section>
  );
}

/** Entretien à prévoir : une ligne par rappel échu, qui ouvre la fiche. */
function MaintenanceBanners() {
  const { t, i18n } = useTranslation();
  const reminders = useReminders((s) => s.reminders);
  const libraries = useImport((s) => s.libraries);
  useEffect(() => {
    void useReminders.getState().load();
  }, [libraries]);
  return reminders
    .filter((r) => r.due)
    .map((r) => (
      <div
        key={`${r.gearId}-${r.kind}`}
        className="banner banner-info welcome-banner"
        role="status"
      >
        <span>
          {t("maintenance.homeDue", {
            name: r.gearName,
            kind: t(`maintenance.kinds.${r.kind}`),
            date: formatDay(r.dueDate, i18n.language),
          })}
        </span>
        <button
          type="button"
          className="button button-small"
          onClick={() => void openMain({ screen: "gear", gearId: r.gearId })}
        >
          {t("maintenance.open")}
        </button>
      </div>
    ));
}

/** Fenêtre d'accueil, ouverte au démarrage. */
/**
 * Photos déjà rangées dans la bibliothèque mais inconnues de la base (base
 * perdue, nouvel ordinateur) : la réanalyse les retrouve, retouches comprises.
 */
function RescanHint() {
  const { t } = useTranslation();
  const libraryId = useImport((s) => s.libraryId);
  const running = useRescan((s) => s.running);
  if (libraryId === null) return null;
  return (
    <p className="welcome-hint">
      <button
        type="button"
        className="button button-small"
        disabled={running}
        onClick={() => void useRescan.getState().run(libraryId)}
      >
        {t(running ? "home.rescanning" : "home.rescan")}
      </button>
    </p>
  );
}

export function WelcomeApp() {
  const { t } = useTranslation();
  useAppEvents("welcome");
  const hasLibrary = useImport((s) => s.libraryId !== null);
  const running = useImport((s) => s.running);
  const resumable = useImport((s) => s.resumable);
  const dropping = useLayout((s) => s.dropping);
  const { recent, highlight } = useWelcomeData();
  const [onStartup, setOnStartup] = useState(welcomeOnStartup);
  const latest = recent[0];

  return (
    <div className="welcome" data-dropping={dropping || undefined}>
      <Hero highlight={highlight} />
      <main className="welcome-scroll">
        <DeviceBanner label="welcome" />
        <OfflineBanner />
        <MaintenanceBanners />
        {resumable.map((r) => (
          <div key={r.id} className="banner banner-info welcome-banner" role="status">
            <span>{t("home.interrupted", { source: r.sourceLabel, count: r.remaining })}</span>
            <button
              type="button"
              className="button button-small"
              disabled={running}
              onClick={() => void openMain({ screen: "import", resumeImportId: r.id })}
            >
              {t("import.resume")}
            </button>
          </div>
        ))}
        {!hasLibrary ? (
          <Onboarding />
        ) : (
          <div className="welcome-columns">
            {latest ? (
              <ContactSheet latest={latest} />
            ) : (
              <section className="welcome-section">
                <header className="welcome-section-header">
                  <h2>{t("home.addPhotos")}</h2>
                </header>
                <p className="welcome-hint">{t("home.dropHint")}</p>
                <RescanHint />
              </section>
            )}
            <AlbumList />
          </div>
        )}
      </main>
      <footer className="welcome-footer">
        <label className="check-row">
          <input
            type="checkbox"
            checked={onStartup}
            onChange={(e) => {
              setOnStartup(e.target.checked);
              setWelcomeOnStartup(e.target.checked);
            }}
          />
          {t("home.showOnStartup")}
        </label>
      </footer>
      <AppDialog />
    </div>
  );
}
