import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type AppInfo, api } from "../api/core";
import defaultPhoto from "../assets/accueil-cabourg.jpg";
import license from "../../LICENSE?raw";
import { systemReport, useAbout } from "../store/about";

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
  { name: "SQLite", license: "publicDomain", version: "sqlite" },
  { name: "Tauri", license: "MIT / Apache 2.0", version: "tauri" },
  { name: "React", license: "MIT" },
  { name: "Lucide", license: "ISC" },
];

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
  const [showLicense, setShowLicense] = useState(false);
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
            <p>{t("about.version", { version: info?.version ?? "0.1.0" })}</p>
            <p className="about-muted">{t("about.copyright")}</p>
          </header>
          {showLicense ? (
            <pre className="about-license" tabIndex={0}>
              {license}
            </pre>
          ) : (
            <section className="about-components">
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
                aria-pressed={showLicense}
                onClick={() => setShowLicense((v) => !v)}
              >
                {showLicense ? t("about.components") : t("about.license")}
              </button>
              <button type="button" className="button" onClick={() => void copy()}>
                {copied ? t("about.copied") : t("about.systemInfo")}
              </button>
            </div>
          </footer>
        </div>
      </div>
    </div>
  );
}
