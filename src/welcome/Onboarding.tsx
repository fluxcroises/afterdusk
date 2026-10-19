import { Check, File, Folder, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toCommandError } from "../api/core";
import { ErrorBanner } from "../components/ErrorBanner";
import { BackupWarning, FolderField, IdentityFields, TemplateChoice } from "../settings/Fields";
import { DEFAULT_FOLDER_TEMPLATE, useSameVolume, useTemplatePreview } from "../settings/presets";
import { useImport } from "../store/import";
import { type OnboardingChoices, applyChoices, defaultRoot } from "../settings/onboarding";

const STEPS = ["library", "backup", "folders", "identity"] as const;
type Step = (typeof STEPS)[number];

/** Arborescence d'exemple : un dossier par ligne, le fichier en dernier. */
function ExampleTree({ path }: { path: string }) {
  const parts = path.split("/");
  return (
    <ul className="summary-tree">
      {parts.map((part, i) => {
        const Icon = i === parts.length - 1 ? File : Folder;
        return (
          <li key={i} style={{ paddingLeft: `${i * 14}px` }}>
            <Icon aria-hidden="true" strokeWidth={1.75} />
            {part}
          </li>
        );
      })}
    </ul>
  );
}

/**
 * Récapitulatif des choix, rempli au fil des étapes : ce qui sera créé, et la
 * confirmation avant « Créer la bibliothèque ». Un clic ramène à l'étape.
 */
function Summary({
  choices: c,
  step,
  onStep,
}: {
  choices: OnboardingChoices;
  step: Step;
  onStep: (step: Step) => void;
}) {
  const { t } = useTranslation();
  const sameDisk = useSameVolume(c.root, c.backup);
  const { path } = useTemplatePreview(
    c.folderTemplate,
    c.renameTemplate,
    t("settings.exampleAlbum"),
  );
  const row = (s: Step, value: React.ReactNode) => (
    <button
      type="button"
      className="summary-row"
      data-current={s === step || undefined}
      disabled={!c.root}
      onClick={() => onStep(s)}
    >
      <span className="summary-label">{t(`onboarding.steps.${s}`)}</span>
      <span className="summary-value">{value}</span>
    </button>
  );
  const identity = [c.identity.name, c.identity.copyright].filter(Boolean).join(" · ");
  return (
    <aside className="onboarding-summary" aria-label={t("onboarding.summary")}>
      <h3>{t("onboarding.summary")}</h3>
      {row(
        "library",
        <>
          <strong>{c.name || t("onboarding.defaultName")}</strong>
          <span className="summary-path">{c.root ?? t("onboarding.toChoose")}</span>
        </>,
      )}
      {row(
        "backup",
        c.backup ? (
          <>
            <span className="summary-path">{c.backup}</span>
            {sameDisk && (
              <span className="summary-warning">
                <TriangleAlert aria-hidden="true" strokeWidth={1.75} />
                {t("onboarding.sameDiskShort")}
              </span>
            )}
          </>
        ) : (
          <span className="summary-muted">{t("onboarding.backup.none")}</span>
        ),
      )}
      {row("folders", path ? <ExampleTree path={path} /> : "—")}
      {row(
        "identity",
        identity ? identity : <span className="summary-muted">{t("onboarding.noIdentity")}</span>,
      )}
    </aside>
  );
}

/**
 * Première ouverture guidée (cahier des charges §6) : bibliothèque, sauvegarde,
 * rangement et identité en quatre écrans, avec des valeurs par défaut.
 */
export function Onboarding() {
  const { t } = useTranslation();
  const [step, setStep] = useState<Step>("library");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ReturnType<typeof toCommandError> | null>(null);
  const [c, setChoices] = useState<OnboardingChoices>({
    name: t("onboarding.defaultName"),
    root: null,
    backup: null,
    folderTemplate: DEFAULT_FOLDER_TEMPLATE,
    renameTemplate: null,
    identity: { name: "", copyright: "", website: "" },
  });
  const update = (patch: Partial<OnboardingChoices>) => setChoices((s) => ({ ...s, ...patch }));

  useEffect(() => {
    let current = true;
    void defaultRoot().then((root) => {
      if (current && root) setChoices((s) => (s.root ? s : { ...s, root }));
    });
    return () => {
      current = false;
    };
  }, []);

  const index = STEPS.indexOf(step);
  const last = index === STEPS.length - 1;

  const finish = async () => {
    setBusy(true);
    setError(null);
    try {
      await applyChoices(c);
      // La bibliothèque existe : l'accueil habituel prend le relais.
      await useImport.getState().loadLibraries();
    } catch (e) {
      setError(toCommandError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="onboarding-layout">
      <section className="onboarding" aria-labelledby="onboarding-title">
        <ol className="onboarding-steps">
          {STEPS.map((s, i) => (
            <li
              key={s}
              aria-current={s === step ? "step" : undefined}
              data-done={i < index || undefined}
            >
              <span className="onboarding-step-number">
                {i < index ? <Check aria-hidden="true" strokeWidth={2.5} /> : i + 1}
              </span>
              {t(`onboarding.steps.${s}`)}
            </li>
          ))}
        </ol>
        <ErrorBanner error={error} onDismiss={() => setError(null)} />
        <div className="onboarding-body">
          <h2 id="onboarding-title">{t(`onboarding.${step}.title`)}</h2>
          <p className="onboarding-text">{t(`onboarding.${step}.text`)}</p>
          {step === "library" && (
            <div className="settings-form">
              <FolderField
                label={t("onboarding.library.folder")}
                path={c.root}
                onChange={(root) => update({ root })}
              />
              <label className="field-row">
                <span className="field-row-label">{t("onboarding.library.name")}</span>
                <input
                  type="text"
                  value={c.name}
                  onChange={(e) => update({ name: e.target.value })}
                />
              </label>
            </div>
          )}
          {step === "backup" && (
            <div className="settings-form">
              <FolderField
                label={t("onboarding.backup.folder")}
                path={c.backup}
                placeholder={t("onboarding.backup.none")}
                onChange={(backup) => update({ backup })}
                onClear={() => update({ backup: null })}
              />
              <BackupWarning root={c.root} backup={c.backup} />
            </div>
          )}
          {step === "folders" && (
            <TemplateChoice
              folderTemplate={c.folderTemplate}
              renameTemplate={c.renameTemplate}
              onChange={(folderTemplate, renameTemplate) =>
                update({ folderTemplate, renameTemplate })
              }
            />
          )}
          {step === "identity" && (
            <IdentityFields identity={c.identity} onChange={(identity) => update({ identity })} />
          )}
        </div>
        <footer className="onboarding-footer">
          {index > 0 && (
            <button
              type="button"
              className="button button-inline"
              disabled={busy}
              onClick={() => setStep(STEPS[index - 1] ?? "library")}
            >
              {t("onboarding.back")}
            </button>
          )}
          <span className="toolbar-spacer" />
          {last ? (
            <button
              type="button"
              className="button button-primary button-inline"
              disabled={busy || !c.root}
              onClick={() => void finish()}
            >
              {t("onboarding.finish")}
            </button>
          ) : (
            <button
              type="button"
              className="button button-primary button-inline"
              disabled={step === "library" && !c.root}
              onClick={() => setStep(STEPS[index + 1] ?? "identity")}
            >
              {t("onboarding.next")}
            </button>
          )}
        </footer>
      </section>
      <Summary choices={c} step={step} onStep={setStep} />
    </div>
  );
}
