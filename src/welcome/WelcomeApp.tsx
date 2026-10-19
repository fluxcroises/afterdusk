import { invoke, isTauri } from "@tauri-apps/api/core";
import { Check, ChevronDown, FolderOpen, Images, Plus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { newLibrary, pickFolder } from "../actions";
import { api, type RecentImport } from "../api/core";
import { previewUrl } from "../api/previews";
import { useAppEvents } from "../appEvents";
import { IconButton } from "../components/IconButton";
import { Thumbnail } from "../components/Thumbnail";
import { WindowControls } from "../components/WindowControls";
import { drawsOwnMenus } from "../platform";
import { useImport } from "../store/import";
import { useLayout } from "../store/layout";
import { type AlbumSummary, useLibrary } from "../store/library";
import { greetingKey } from "../ui/greeting";
import { openMain, setWelcomeOnStartup, welcomeOnStartup } from "../windows";
import "../App.css";

type Tab = "home" | "albums";

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

function Header() {
  const { t } = useTranslation();
  const hasLibrary = useImport((s) => s.libraryId !== null);
  return (
    <header className="welcome-header" data-tauri-drag-region>
      <span className="welcome-logo" aria-hidden="true" />
      <div className="welcome-identity" data-tauri-drag-region>
        <h1 className="welcome-greeting">{t(greetingKey())}</h1>
        <LibrarySwitcher />
      </div>
      <span className="toolbar-spacer" data-tauri-drag-region />
      <div className="welcome-actions">
        <IconButton
          icon={Images}
          label={t("modules.library")}
          disabled={!hasLibrary}
          onClick={() => void openMain({ screen: "library", albumId: null })}
        />
        <IconButton
          icon={FolderOpen}
          label={t("menu.importFolder")}
          disabled={!hasLibrary}
          onClick={() => void importFolder(t("import.chooseFolder"))}
        />
        <button
          type="button"
          className="welcome-primary"
          aria-label={t("modules.import")}
          title={t("modules.import")}
          disabled={!hasLibrary}
          onClick={() => void openMain({ screen: "import" })}
        >
          <Plus aria-hidden="true" strokeWidth={2} />
        </button>
      </div>
    </header>
  );
}

/** Photo d'illustration, tirée des meilleures photos de la bibliothèque. */
function Hero({ highlight }: { highlight: Highlight | null }) {
  const { i18n } = useTranslation();
  const date = formatDate(highlight?.capturedAt ?? null, i18n.language);
  return (
    <section className="welcome-hero" aria-hidden={!highlight}>
      {highlight ? (
        <>
          <img src={previewUrl(highlight.mediaId, 2048) ?? undefined} alt="" decoding="async" />
          <span className="welcome-hero-caption">
            {[highlight.albumName, date].filter(Boolean).join(" · ")}
          </span>
        </>
      ) : (
        <span className="welcome-hero-mark" />
      )}
    </section>
  );
}

function Row({
  title,
  more,
  onMore,
  children,
}: {
  title: string;
  more?: string;
  onMore?: () => void;
  children: React.ReactNode;
}) {
  return (
    <section className="welcome-row">
      <header className="welcome-row-header">
        <h2>{title}</h2>
        {more && onMore && (
          <button type="button" className="link-button" onClick={onMore}>
            {more}
          </button>
        )}
      </header>
      {children}
    </section>
  );
}

function AlbumCard({ album }: { album: AlbumSummary }) {
  const { t, i18n } = useTranslation();
  const versions = useLibrary((s) => s.versions);
  const cover = album.coverMediaId;
  return (
    <button
      type="button"
      className="album-card"
      onClick={() => void openMain({ screen: "cull", albumId: album.id })}
    >
      <span className="album-card-cover">
        <Thumbnail src={cover === null ? null : previewUrl(cover, 1024, versions[cover] ?? 0)} />
      </span>
      <span className="album-card-name">{album.name}</span>
      <span className="album-card-meta">
        {[
          formatDate(album.startDate, i18n.language),
          t("toolbar.photoCount", { count: album.count }),
        ]
          .filter(Boolean)
          .join(" · ")}
      </span>
    </button>
  );
}

function HomeTab({ recent, onAlbums }: { recent: RecentImport[]; onAlbums: () => void }) {
  const { t } = useTranslation();
  const albums = useLibrary((s) => s.albums);
  const versions = useLibrary((s) => s.versions);
  const latest = recent[0];

  return (
    <>
      {latest && (
        <Row
          title={t("home.lastImportTitle", { source: latest.sourceLabel })}
          more={t("home.more")}
          onMore={() => void openMain({ screen: "cull" })}
        >
          <div className="welcome-strip">
            {latest.mediaIds.map((id) => (
              <button
                key={id}
                type="button"
                className="welcome-strip-cell"
                onClick={() => void openMain({ screen: "cull", mediaId: id })}
              >
                <Thumbnail src={previewUrl(id, 256, versions[id] ?? 0)} />
              </button>
            ))}
          </div>
        </Row>
      )}
      {albums.length > 0 && (
        <Row title={t("home.recentAlbums")} more={t("home.more")} onMore={onAlbums}>
          <div className="welcome-strip">
            {albums.slice(0, 8).map((a) => (
              <AlbumCard key={a.id} album={a} />
            ))}
          </div>
        </Row>
      )}
      {!latest && albums.length === 0 && (
        <Row title={t("home.addPhotos")}>
          <p className="welcome-hint">{t("home.dropHint")}</p>
        </Row>
      )}
    </>
  );
}

function FirstSteps() {
  const { t } = useTranslation();
  return (
    <div className="welcome-first">
      <h2>{t("home.firstStepsTitle")}</h2>
      <p>{t("home.firstSteps")}</p>
      <button
        type="button"
        className="button button-primary button-inline"
        onClick={() => void newLibrary()}
      >
        {t("import.newLibrary")}
      </button>
    </div>
  );
}

/** Fenêtre d'accueil, ouverte au démarrage. */
export function WelcomeApp() {
  const { t } = useTranslation();
  useAppEvents("welcome");
  const hasLibrary = useImport((s) => s.libraryId !== null);
  const running = useImport((s) => s.running);
  const resumable = useImport((s) => s.resumable);
  const albums = useLibrary((s) => s.albums);
  const dropping = useLayout((s) => s.dropping);
  const { recent, highlight } = useWelcomeData();
  const [tab, setTab] = useState<Tab>("home");
  const [onStartup, setOnStartup] = useState(welcomeOnStartup);

  return (
    <div className="welcome" data-dropping={dropping || undefined}>
      <div className="welcome-titlebar" data-tauri-drag-region>
        {drawsOwnMenus && <WindowControls />}
      </div>
      <Header />
      <nav className="welcome-tabs" role="tablist" aria-label={t("app.name")}>
        {(["home", "albums"] as const).map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            className="welcome-tab"
            onClick={() => setTab(id)}
          >
            {t(id === "home" ? "modules.home" : "catalog.albums")}
          </button>
        ))}
      </nav>
      <main className="welcome-scroll">
        {tab === "home" && <Hero highlight={highlight} />}
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
          <FirstSteps />
        ) : tab === "home" ? (
          <HomeTab recent={recent} onAlbums={() => setTab("albums")} />
        ) : (
          <div className="album-grid">
            {albums.map((a) => (
              <AlbumCard key={a.id} album={a} />
            ))}
            {albums.length === 0 && <p className="welcome-hint">{t("catalog.noAlbums")}</p>}
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
    </div>
  );
}
