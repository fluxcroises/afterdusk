import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type CommandError, toCommandError } from "../api/core";
import { ErrorBanner } from "../components/ErrorBanner";
import { FieldRow } from "../components/PanelSection";
import { useImport } from "../store/import";
import { confirmWarning } from "../ui/confirm";

/** Miroir de `ftp::FtpSettings`. */
interface FtpSettings {
  enabled: boolean;
  port: number;
  user: string;
  password: string;
  allowPlain: boolean;
  libraryId: number | null;
  albumName: string;
}

/** Miroir de `commands::ftp::FtpInfo`. */
interface FtpInfo {
  settings: FtpSettings;
  status: { running: boolean; error: string | null };
  address: string | null;
  reception: string;
  fingerprint: string | null;
}

/**
 * Réception FTP sur le réseau local (désactivée par défaut) : ce qu'il faut
 * saisir dans le boîtier, le chiffrement, le certificat et la réception.
 */
export function FtpReception() {
  const { t } = useTranslation();
  const libraryId = useImport((s) => s.libraryId);
  const [info, setInfo] = useState<FtpInfo | null>(null);
  const [draft, setDraft] = useState<FtpSettings | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const load = () =>
    invoke<FtpInfo>("ftp_get")
      .then((i) => {
        if (!i?.settings) return;
        setInfo(i);
        setDraft(i.settings);
      })
      .catch((e: unknown) => setError(toCommandError(e)));
  useEffect(() => {
    void load();
  }, []);
  if (!info || !draft) return null;

  const apply = async (next: FtpSettings) => {
    setDraft(next);
    try {
      await invoke("ftp_set", { settings: next });
      setError(null);
    } catch (e) {
      setError(toCommandError(e));
    }
    await load();
  };
  const field = (key: "user" | "albumName") => ({
    value: draft[key],
    onChange: (e: { currentTarget: { value: string } }) =>
      setDraft({ ...draft, [key]: e.currentTarget.value }),
    onBlur: () => {
      if (draft[key] !== info.settings[key]) void apply(draft);
    },
  });

  const saveCertificate = async () => {
    const path = await save({
      title: t("settings.ftpCertificateSave"),
      defaultPath: "afterdusk.cer",
      filters: [{ name: t("settings.ftpCertificate"), extensions: ["cer"] }],
    });
    if (!path) return;
    await invoke("ftp_save_certificate", { path }).catch((e: unknown) =>
      setError(toCommandError(e)),
    );
  };

  const clear = async () => {
    const ok = await confirmWarning(t("settings.ftpClearConfirm"), {
      title: t("settings.ftp"),
      okLabel: t("settings.ftpClear"),
      cancelLabel: t("trash.cancel"),
    });
    if (!ok) return;
    try {
      const [removed, kept] = await invoke<[number, number]>("ftp_clear_reception");
      setNote(
        t("settings.ftpCleared", { count: removed, kept: t("settings.ftpKept", { count: kept }) }),
      );
    } catch (e) {
      setError(toCommandError(e));
    }
  };

  const enabled = info.settings.enabled;
  return (
    <div className="ftp-reception">
      <h3 className="settings-block-title">{t("settings.ftp")}</h3>
      <ErrorBanner error={error} onDismiss={() => setError(null)} />
      <label className="check-row">
        <input
          type="checkbox"
          checked={enabled}
          disabled={libraryId === null}
          onChange={(e) =>
            void apply({
              ...draft,
              enabled: e.currentTarget.checked,
              libraryId: draft.libraryId ?? libraryId,
              albumName: draft.albumName || t("settings.ftpDefaultAlbum"),
            })
          }
        />
        {t("settings.ftpEnable")}
      </label>
      {enabled && (
        <>
          <p className="panel-note" role="status">
            {info.status.running
              ? t("settings.ftpRunning")
              : t("settings.ftpStopped", { error: info.status.error ?? "" })}
          </p>
          <FieldRow
            label={t("settings.ftpAddress")}
            value={info.address ?? t("settings.ftpNoAddress")}
          />
          <FieldRow label={t("settings.ftpPort")} value={String(info.settings.port)} />
          <label className="field-row">
            <span className="field-row-label">{t("settings.ftpUser")}</span>
            <input type="text" maxLength={40} {...field("user")} />
          </label>
          <div className="field-row">
            <span className="field-row-label">{t("settings.ftpPassword")}</span>
            <code className="ftp-secret">{info.settings.password}</code>
            <button
              type="button"
              className="button button-small"
              onClick={() => void apply({ ...draft, password: "" })}
            >
              {t("settings.ftpNewPassword")}
            </button>
          </div>
          <FieldRow label={t("settings.ftpRemoteFolder")} value="/" />
          <label className="field-row">
            <span className="field-row-label">{t("settings.ftpAlbum")}</span>
            <input type="text" maxLength={120} {...field("albumName")} />
          </label>
          <label className="check-row">
            <input
              type="checkbox"
              checked={info.settings.allowPlain}
              onChange={(e) => void apply({ ...draft, allowPlain: e.currentTarget.checked })}
            />
            {t("settings.ftpAllowPlain")}
          </label>
          {info.settings.allowPlain && (
            <p className="ftp-warning">
              <TriangleAlert aria-hidden="true" strokeWidth={1.75} />
              {t("settings.ftpPlainWarning")}
            </p>
          )}
          {info.fingerprint && (
            <div className="field-row ftp-fingerprint">
              <span className="field-row-label">{t("settings.ftpFingerprint")}</span>
              <code>{info.fingerprint}</code>
            </div>
          )}
          <div className="settings-actions">
            <button
              type="button"
              className="button button-small"
              onClick={() => void saveCertificate()}
            >
              {t("settings.ftpCertificateSave")}
            </button>
            <button type="button" className="button button-small" onClick={() => void clear()}>
              {t("settings.ftpClear")}
            </button>
          </div>
          {note && <p className="panel-note">{note}</p>}
          <p className="panel-note">{t("settings.ftpNote")}</p>
        </>
      )}
    </div>
  );
}
