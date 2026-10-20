import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { previewUrl } from "../api/previews";
import { useDeliver } from "../store/deliver";
import { useLibrary } from "../store/library";
import { PanelSection } from "./PanelSection";

/** Délai avant de refaire l'aperçu après une modification. */
const PREVIEW_DELAY_MS = 350;

interface Result {
  width: number;
  height: number;
  /** Adresse de l'image (objet Blob), ou rien pour les originaux. */
  url: string | null;
}

/**
 * Aperçu du résultat (cahier des charges, écran Livrer) : la photo active
 * telle que le preset choisi l'exportera (recadrage, retouche, filigrane), et
 * la taille du fichier.
 */
export function ResultPreview() {
  const { t } = useTranslation();
  const settings = useDeliver((s) => s.settings);
  const chosen = useDeliver((s) => s.chosen);
  const item = useLibrary((s) => s.selectedId);
  const version = useLibrary((s) => (s.selectedId === null ? 0 : (s.versions[s.selectedId] ?? 0)));
  const [picked, setPicked] = useState<string | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);

  const presets = (settings?.presets ?? []).filter((p) => chosen.includes(p.id));
  const preset = presets.find((p) => p.id === picked) ?? presets[0];
  const watermark = settings?.watermark ?? null;
  const key = JSON.stringify([item, preset, watermark, version]);

  useEffect(() => {
    if (item === null || !preset) return;
    let cancelled = false;
    let url: string | null = null;
    const timer = setTimeout(() => {
      setLoading(true);
      setFailed(false);
      invoke<ArrayBuffer>("export_preview", { item, preset, watermark })
        .then((buffer) => {
          if (cancelled) return;
          const view = new DataView(buffer);
          const jpeg = buffer.byteLength > 8 ? buffer.slice(8) : null;
          url = jpeg ? URL.createObjectURL(new Blob([jpeg], { type: "image/jpeg" })) : null;
          setResult({ width: view.getUint32(0, true), height: view.getUint32(4, true), url });
        })
        .catch(() => !cancelled && setFailed(true))
        .finally(() => !cancelled && setLoading(false));
    }, PREVIEW_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      if (url) URL.revokeObjectURL(url);
    };
    // `key` résume la photo, le preset et le filigrane.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const nameOf = (id: string, name: string | null) =>
    name ?? t(`deliver.presets.${id}`, { defaultValue: id });
  const original = preset?.output.kind === "original";
  const fallback = item === null ? null : previewUrl(item, 1024, version);

  return (
    <PanelSection id="deliver.preview" title={t("deliver.preview")}>
      {!preset || item === null ? (
        <p className="panel-note">
          {t(item === null ? "deliver.needSelection" : "deliver.needPreset")}
        </p>
      ) : (
        <>
          {presets.length > 1 && (
            <label className="deliver-field">
              <span>{t("deliver.previewPreset")}</span>
              <select value={preset.id} onChange={(e) => setPicked(e.currentTarget.value)}>
                {presets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {nameOf(p.id, p.name)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <div className="deliver-preview" aria-busy={loading}>
            {original ? (
              fallback && <img src={fallback} alt="" />
            ) : result?.url && !failed ? (
              <img src={result.url} alt={t("deliver.preview")} data-stale={loading || undefined} />
            ) : (
              <span className="panel-note">
                {t(failed ? "deliver.previewFailed" : "deliver.previewLoading")}
              </span>
            )}
          </div>
          <p className="panel-note">
            {original
              ? t("deliver.originalHint")
              : result && result.width > 0
                ? t("deliver.previewSize", { width: result.width, height: result.height })
                : " "}
          </p>
        </>
      )}
    </PanelSection>
  );
}
