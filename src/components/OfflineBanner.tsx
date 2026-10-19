import { isTauri } from "@tauri-apps/api/core";
import { HardDrive, Unplug } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type OfflineInfo, api } from "../api/core";
import { useImport } from "../store/import";

/** Durée du message « de nouveau disponible ». */
export const BACK_ONLINE_MS = 6000;

/** Intervalle de mise à jour du nombre de notes en attente, hors ligne. */
const INFO_REFRESH_MS = 3000;

/**
 * Bibliothèque sur un disque débranché (§4.2) : elle reste consultable grâce
 * aux aperçus en cache. Le bandeau dit ce qui attend le disque, et annonce
 * son retour.
 */
export function OfflineBanner() {
  const { t, i18n } = useTranslation();
  const library = useImport((s) => s.libraries.find((l) => l.id === s.libraryId) ?? null);
  const offline = library !== null && !library.isOnline;
  const [info, setInfo] = useState<OfflineInfo | null>(null);
  const [wasOffline, setWasOffline] = useState<number | null>(null);
  const [backOnline, setBackOnline] = useState<string | null>(null);

  // Retour du disque : un message bref, puis plus rien.
  if (offline && library && wasOffline !== library.id) setWasOffline(library.id);
  if (!offline && wasOffline !== null) {
    setWasOffline(null);
    if (library && library.id === wasOffline) setBackOnline(library.name);
  }

  useEffect(() => {
    if (!backOnline) return;
    const timer = window.setTimeout(() => setBackOnline(null), BACK_ONLINE_MS);
    return () => window.clearTimeout(timer);
  }, [backOnline]);

  useEffect(() => {
    if (!offline || !library || !isTauri()) return;
    let current = true;
    const load = () =>
      void api
        .libraryOfflineInfo(library.id)
        .then((i) => current && setInfo(i))
        .catch(() => undefined);
    load();
    const timer = window.setInterval(load, INFO_REFRESH_MS);
    return () => {
      current = false;
      window.clearInterval(timer);
    };
  }, [offline, library]);

  if (backOnline) {
    return (
      <div className="banner banner-info offline-banner" role="status">
        <HardDrive aria-hidden="true" strokeWidth={1.75} />
        <span>{t("offline.back", { name: backOnline })}</span>
      </div>
    );
  }
  if (!offline || !library) return null;
  const lastSeen = info?.lastSeen
    ? new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(
        new Date(info.lastSeen),
      )
    : null;
  return (
    <div className="banner banner-warning offline-banner" role="status">
      <Unplug aria-hidden="true" strokeWidth={1.75} />
      <span>
        <strong>{t("offline.title", { name: library.name })}</strong>{" "}
        {t("offline.text", { root: library.rootPath })}
        {info && info.pendingXmp > 0 && <> {t("offline.pending", { count: info.pendingXmp })}</>}
        {lastSeen && (
          <span className="offline-last-seen"> {t("offline.lastSeen", { date: lastSeen })}</span>
        )}
      </span>
    </div>
  );
}
