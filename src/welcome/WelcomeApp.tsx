import { isTauri } from "@tauri-apps/api/core";
import {
  ChevronRight,
  FolderPlus,
  HardDriveDownload,
  Images,
  type LucideIcon,
  Share,
  SlidersHorizontal,
  SquareCheckBig,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { newLibrary, pickFolder } from "../actions";
import { api, type RecentImport } from "../api/core";
import { previewUrl } from "../api/previews";
import { useAppEvents } from "../appEvents";
import { ListRow, PanelSection } from "../components/PanelSection";
import { Thumbnail } from "../components/Thumbnail";
import { useImport } from "../store/import";
import { useLayout } from "../store/layout";
import { useLibrary } from "../store/library";
import { AVAILABLE_IN, CURRENT_VERSION, type Step } from "../store/navigation";
import { greetingKey } from "../ui/greeting";
import { openMain, setWelcomeOnStartup, welcomeOnStartup } from "../windows";
import "../App.css";

const JOURNEY: { step: Step; icon: LucideIcon }[] = [
  { step: "import", icon: HardDriveDownload },
  { step: "cull", icon: SquareCheckBig },
  { step: "develop", icon: SlidersHorizontal },
  { step: "deliver", icon: Share },
];

function useRecentImports() {
  const libraryId = useImport((s) => s.libraryId);
  const libraries = useImport((s) => s.libraries);
  const [recent, setRecent] = useState<RecentImport[]>([]);
  useEffect(() => {
    if (!isTauri() || libraryId === null) return;
    let current = true;
    api
      .importsRecent(libraryId)
      .then((r) => current && setRecent(r))
      .catch(() => current && setRecent([]));
    return () => {
      current = false;
    };
    // `libraries` change à chaque rafraîchissement de l'accueil.
  }, [libraryId, libraries]);
  return libraryId === null ? [] : recent;
}

async function importFolder(title: string) {
  const path = await pickFolder(title);
  if (path) await openMain({ screen: "import", scanPath: path });
}

function RecentActivity({ recent }: { recent: RecentImport[] }) {
  const { t, i18n } = useTranslation();
  const versions = useLibrary((s) => s.versions);
  const latest = recent[0];
  if (!latest) return null;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "long" }).format(
    new Date(latest.date),
  );

  return (
    <section className="home-section">
      <h2 className="home-heading">{t("home.recent")}</h2>
      <div className="home-card">
        <div className="home-card-header">
          <span>
            {t("home.lastImport", { source: latest.sourceLabel, date })}
            <span className="list-row-count">
              {" "}
              · {t("toolbar.photoCount", { count: latest.count })}
            </span>
          </span>
          <button
            type="button"
            className="link-button"
            onClick={() => void openMain({ screen: "library", albumId: null })}
          >
            <Images aria-hidden="true" strokeWidth={1.75} />
            {t("home.viewAll")}
          </button>
        </div>
        <div className="home-strip">
          {latest.mediaIds.map((id) => (
            <button
              key={id}
              type="button"
              className="home-strip-cell"
              onClick={() => void openMain({ screen: "cull", mediaId: id })}
            >
              <Thumbnail src={previewUrl(id, 256, versions[id] ?? 0)} />
            </button>
          ))}
          {latest.count > latest.mediaIds.length && (
            <button
              type="button"
              className="home-strip-more"
              onClick={() => void openMain({ screen: "cull" })}
            >
              <ChevronRight aria-hidden="true" />
              <span className="visually-hidden">{t("home.viewAll")}</span>
            </button>
          )}
        </div>
      </div>
    </section>
  );
}

function AddPhotos() {
  const { t } = useTranslation();
  const hasLibrary = useImport((s) => s.libraries.length > 0);

  if (!hasLibrary) {
    return (
      <section className="home-section">
        <h2 className="home-heading">{t("home.firstStepsTitle")}</h2>
        <div className="drop-zone">
          <FolderPlus className="drop-zone-icon" aria-hidden="true" strokeWidth={1.25} />
          <p>{t("home.firstSteps")}</p>
          <button
            type="button"
            className="button button-primary button-inline"
            onClick={() => void newLibrary()}
          >
            {t("import.newLibrary")}
          </button>
        </div>
      </section>
    );
  }

  return (
    <section className="home-section">
      <h2 className="home-heading">{t("home.addPhotos")}</h2>
      <div className="drop-zone">
        <HardDriveDownload className="drop-zone-icon" aria-hidden="true" strokeWidth={1.25} />
        <p>{t("home.dropHint")}</p>
        <button
          type="button"
          className="button button-primary button-inline"
          onClick={() => void importFolder(t("import.chooseFolder"))}
        >
          {t("menu.importFolder")}
        </button>
      </div>
    </section>
  );
}

function Journey() {
  const { t } = useTranslation();
  return (
    <section className="home-section">
      <h2 className="home-heading">{t("home.journey")}</h2>
      <p className="home-subheading">{t("home.journeyText")}</p>
      <div className="journey">
        {JOURNEY.map(({ step, icon: Icon }, index) => {
          const later = AVAILABLE_IN[step] !== CURRENT_VERSION;
          return (
            <button
              key={step}
              type="button"
              className="journey-card"
              data-later={later || undefined}
              onClick={() => void openMain({ screen: step })}
            >
              <span className="journey-art" aria-hidden="true">
                <Icon strokeWidth={1} />
                <span className="journey-index">{index + 1}</span>
              </span>
              <span className="journey-title">{t(`modules.${step}`)}</span>
              <span className="journey-text">{t(`home.steps.${step}`)}</span>
              <span className="journey-meta">
                {later
                  ? t("modules.availableIn", { version: AVAILABLE_IN[step] })
                  : t("home.available")}
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

function Sidebar() {
  const { t } = useTranslation();
  const total = useLibrary((s) => s.total);
  const albums = useLibrary((s) => s.albums);

  return (
    <aside className="welcome-sidebar" aria-label={t("panels.left")}>
      <div className="welcome-brand">
        <span className="logo" aria-hidden="true" />
        <span className="brand-name">{t("app.name")}</span>
      </div>
      <div className="home-actions">
        <button
          type="button"
          className="button button-primary"
          onClick={() => void openMain({ screen: "import" })}
        >
          {t("modules.import")}
        </button>
      </div>
      <div className="side-panel-scroll">
        <PanelSection id="home.library" title={t("home.yourPhotos")}>
          <ListRow
            label={t("catalog.allPhotos")}
            count={total}
            onClick={() => void openMain({ screen: "library", albumId: null })}
          />
        </PanelSection>
        <PanelSection id="home.albums" title={t("home.recentAlbums")}>
          {albums.slice(0, 10).map((a) => (
            <ListRow
              key={a.id}
              label={a.name}
              count={a.count}
              onClick={() => void openMain({ screen: "cull", albumId: a.id })}
            />
          ))}
          {albums.length === 0 && <p className="panel-note">{t("catalog.noAlbums")}</p>}
        </PanelSection>
      </div>
    </aside>
  );
}

/** Fenêtre d'accueil, ouverte au démarrage (cahier des charges, section 6). */
export function WelcomeApp() {
  const { t } = useTranslation();
  useAppEvents("welcome");
  const running = useImport((s) => s.running);
  const resumable = useImport((s) => s.resumable);
  const dropping = useLayout((s) => s.dropping);
  const recent = useRecentImports();
  const [onStartup, setOnStartup] = useState(welcomeOnStartup);

  return (
    <div className="welcome" data-dropping={dropping || undefined}>
      <Sidebar />
      <div className="welcome-main">
        <main className="welcome-scroll">
          <div className="home">
            <h1 className="home-title">{t(greetingKey())}</h1>
            <p className="home-intro">{t("home.intro")}</p>

            {resumable.map((r) => (
              <div key={r.id} className="banner banner-info" role="status">
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

            <RecentActivity recent={recent} />
            <AddPhotos />
            <Journey />
          </div>
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
          <span className="toolbar-spacer" />
          <span>{t("status.safety")}</span>
        </footer>
      </div>
    </div>
  );
}
