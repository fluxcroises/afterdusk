import { HardDriveDownload } from "lucide-react";
import { useTranslation } from "react-i18next";
import { deviceLabel, useDevices } from "../store/devices";
import { useImport } from "../store/import";
import type { WindowLabel } from "../windows";

/** « Carte FUJI (E:) détectée · Importer » : l'import proposé à l'insertion. */
export function DeviceBanner({ label }: { label: WindowLabel }) {
  const { t } = useTranslation();
  const pending = useDevices((s) => s.pending);
  const running = useImport((s) => s.running);
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
