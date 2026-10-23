import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import { type AppInfo, api } from "../api/core";
import defaultPhoto from "../assets/accueil-cabourg.jpg";
import publisherLogo from "../assets/logo_horizontal_blanc.png";
import license from "../../LICENSE?raw";
import { version as packageVersion } from "../../package.json";
import { systemReport, useAbout } from "../store/about";
import { useUpdates } from "../store/updates";

/**
 * Composants livrés avec Afterdusk et leur licence (obligation des licences
 * libres). Les noms de licence ne se traduisent pas, sauf « domaine public ».
 */
const COMPONENTS: { name: string; license: string; version?: keyof AppInfo }[] = [
  { name: "LibRaw", license: "LGPL 2.1", version: "libraw" },
  { name: "Adobe XMP Toolkit", license: "BSD 3" },
  { name: "lensfun (portage Rust)", license: "LGPL 3" },
  { name: "Base lensfun", license: "CC BY-SA 3.0" },
  { name: "GeoNames", license: "CC BY 4.0" },
  { name: "MapLibre GL JS", license: "BSD 3" },
  { name: "Natural Earth", license: "publicDomain" },
  { name: "jpeg-encoder", license: "MIT / Apache 2.0, IJG" },
  { name: "rav1e, ravif", license: "BSD" },
  { name: "printpdf", license: "MIT" },
  { name: "notify", license: "CC0 1.0" },
  { name: "FFmpeg", license: "GPL 2.0+" },
  { name: "libunftp", license: "Apache 2.0" },
  { name: "rcgen, rustls, ring", license: "MIT / Apache 2.0, ISC" },
  { name: "ONNX Runtime", license: "MIT" },
  { name: "YuNet (OpenCV Zoo)", license: "MIT" },
  { name: "MediaPipe Face Mesh", license: "Apache 2.0" },
  { name: "Inter", license: "SIL OFL 1.1" },
  { name: "Caveat, Playfair Display, Cormorant Garamond, Bebas Neue", license: "SIL OFL 1.1" },
  { name: "fontdb", license: "MIT" },
  { name: "Konva, react-konva", license: "MIT" },
  { name: "SQLite", license: "publicDomain", version: "sqlite" },
  { name: "Tauri", license: "MIT / Apache 2.0", version: "tauri" },
  { name: "React", license: "MIT" },
  { name: "Lucide", license: "ISC" },
];

/** Recherche des mises à jour à la demande, sur la ligne de la version. */
function UpdateCheck() {
  const { t } = useTranslation();
  const manual = useUpdates((s) => s.manual);
  const available = useUpdates((s) => s.available);
  const progress = useUpdates((s) => s.progress);
  const { checkNow, install } = useUpdates.getState();
  const link = (label: string, run: () => void) => (
    <button type="button" className="about-site-link" onClick={run}>
      {label}
    </button>
  );
  if (progress) return <span role="status">{t("updates.downloading")}</span>;
  if (available) {
    return (
      <span role="status">
        {t("about.updateAvailable", { version: available.version })}{" "}
        {link(t("updates.install"), () => void install())}
      </span>
    );
  }
  if (manual === "checking") return <span role="status">{t("about.updateChecking")}</span>;
  if (manual === "upToDate") return <span role="status">{t("about.upToDate")}</span>;
  if (manual === "failed") {
    return (
      <span role="status">
        {t("about.updateFailed")} {link(t("about.updateRetry"), () => void checkNow())}
      </span>
    );
  }
  return link(t("about.updateCheck"), () => void checkNow());
}

/**
 * Aide > À propos d'Afterdusk, sur le modèle des boîtes « À propos » des
 * logiciels de bureau : une bande photo à gauche avec le logo, à droite la
 * version, les composants et l'avis de licence, les boutons à droite en bas.
 */
export function AboutDialog() {
  const { t, i18n } = useTranslation();
  const open = useAbout((s) => s.open);
  const close = useAbout((s) => s.close);
  const [info, setInfo] = useState<AppInfo | null>(null);
  // Partie du milieu : composants, licence ou informations système, à hauteur fixe.
  const [view, setView] = useState<"components" | "license" | "system">("components");
  const [copied, setCopied] = useState(false);
  const ok = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    ok.current?.focus();
    if (!isTauri()) return;
    let current = true;
    api
      .appInfo()
      .then((i) => current && setInfo(i))
      .catch(() => undefined);
    const escape = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", escape);
    return () => {
      current = false;
      window.removeEventListener("keydown", escape);
    };
  }, [open, close]);

  if (!open) return null;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(systemReport(info, i18n.language));
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div
        className="about-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="about-title"
        onKeyDown={(e) => e.key === "Escape" && close()}
      >
        <div className="about-strip" aria-hidden="true">
          <img src={defaultPhoto} alt="" />
          <span className="logo about-logo" />
        </div>
        <div className="about-body">
          <header>
            <h2 id="about-title" className="about-title">
              {t("app.name")}
            </h2>
            <p>
              {t("about.version", { version: info?.version ?? packageVersion })}
              {isTauri() && (
                <>
                  {" · "}
                  <UpdateCheck />
                </>
              )}
            </p>
            <p className="about-muted">{t("about.copyright")}</p>
            <p className="about-muted">
              <Trans
                i18nKey="about.publisher"
                components={{
                  site: (
                    <button
                      type="button"
                      className="about-site-link"
                      onClick={() => void api.openPublisherSite().catch(() => undefined)}
                    />
                  ),
                }}
              />
            </p>
            <img className="about-publisher-logo" src={publisherLogo} alt="" />
          </header>
          {view === "license" ? (
            <pre className="about-panel about-text" tabIndex={0}>
              {license}
            </pre>
          ) : view === "system" ? (
            <div className="about-panel about-system">
              <pre className="about-text" tabIndex={0}>
                {systemReport(info, i18n.language)}
              </pre>
              <button type="button" className="button button-small" onClick={() => void copy()}>
                {copied ? t("about.copied") : t("about.copy")}
              </button>
            </div>
          ) : (
            <section className="about-panel about-components">
              {/* Un paragraphe discret, comme les mentions des « À propos » classiques. */}
              <p>
                {t("about.componentsLead")}{" "}
                {COMPONENTS.map((c) => {
                  // « 0.22.2-Release » : seul le numéro intéresse.
                  const version =
                    c.version && info ? String(info[c.version] ?? "").replace(/-Release$/, "") : "";
                  const license =
                    c.license === "publicDomain" ? t("about.publicDomain") : c.license;
                  return `${c.name}${version ? ` ${version}` : ""} (${license})`;
                }).join(" · ")}
                .
              </p>
              <p>{t("about.photo")}</p>
            </section>
          )}
          <footer className="about-footer">
            <p className="about-notice">{t("about.notice")}</p>
            <div className="about-buttons">
              <button
                ref={ok}
                type="button"
                className="button button-primary"
                onClick={() => close()}
              >
                {t("about.ok")}
              </button>
              <button
                type="button"
                className="button"
                aria-pressed={view === "license"}
                onClick={() => setView((v) => (v === "license" ? "components" : "license"))}
              >
                {view === "license" ? t("about.components") : t("about.license")}
              </button>
              <button
                type="button"
                className="button"
                aria-pressed={view === "system"}
                onClick={() => setView((v) => (v === "system" ? "components" : "system"))}
              >
                {view === "system" ? t("about.components") : t("about.systemInfo")}
              </button>
            </div>
          </footer>
        </div>
      </div>
    </div>
  );
}
