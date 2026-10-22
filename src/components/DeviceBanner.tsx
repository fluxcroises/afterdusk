import { HardDriveDownload } from "lucide-react";
import { useTranslation } from "react-i18next";
import { deviceLabel, useDevices } from "../store/devices";
import { useImport } from "../store/import";
import type { WindowLabel } from "../windows";

/** Carte d'un boîtier connu : import automatique en cours, puis son bilan. */
function AutoImportBanner() {
  const { t } = useTranslation();
  const auto = useDevices((s) => s.auto);
  const progress = useImport((s) => s.progress);
  const lastResult = useImport((s) => s.lastResult);
  if (!auto) return null;
  let text: string;
  if (auto.kind === "nothingNew") {
    text = t("devices.autoNothingNew", { camera: auto.camera });
  } else if (lastResult?.importId === auto.importId) {
    text = lastResult.error
      ? t("devices.autoFailed", { camera: auto.camera })
      : t("devices.autoDone", { count: progress?.imported ?? 0, camera: auto.camera });
  } else {
    text = t("devices.autoRunning", {
      camera: auto.camera,
      done: progress?.processed ?? 0,
      total: progress?.total ?? auto.files,
    });
  }
  return (
    <div className="banner banner-info device-banner" role="status">
      <HardDriveDownload aria-hidden="true" strokeWidth={1.75} />
      <span>{text}</span>
      <button
        type="button"
        className="button button-small"
        onClick={() => useDevices.getState().dismissAuto()}
      >
        {t("import.dismiss")}
      </button>
    </div>
  );
}

/** « Carte FUJI (E:) détectée · Importer » : l'import proposé à l'insertion. */
export function DeviceBanner({ label }: { label: WindowLabel }) {
  const { t } = useTranslation();
  const pending = useDevices((s) => s.pending);
  const running = useImport((s) => s.running);
  const auto = useDevices((s) => s.auto);
  if (auto) return <AutoImportBanner />;
  if (!pending) return null;
  return (
    <div className="banner banner-info device-banner" role="status">
      <HardDriveDownload aria-hidden="true" strokeWidth={1.75} />
      <span>{t("devices.detected", { device: deviceLabel(pending) })}</span>
      <button
        type="button"
        className="button button-small button-primary"
        disabled={running}
        onClick={() => void useDevices.getState().openImport(pending, label)}
      >
        {t("devices.import")}
      </button>
      <button
        type="button"
        className="button button-small"
        onClick={() => useDevices.getState().dismiss()}
      >
        {t("import.dismiss")}
      </button>
    </div>
  );
}
