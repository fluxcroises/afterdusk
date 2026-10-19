import { Check, FolderTree, HardDrive, Images, type LucideIcon, UserRound } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toCommandError } from "../api/core";
import { ErrorBanner } from "../components/ErrorBanner";
import { BackupWarning, FolderField, IdentityFields, TemplateChoice } from "../settings/Fields";
import { DEFAULT_FOLDER_TEMPLATE } from "../settings/presets";
import { useImport } from "../store/import";
import { type OnboardingChoices, applyChoices, defaultRoot } from "../settings/onboarding";

const STEPS = ["library", "backup", "folders", "identity"] as const;
type Step = (typeof STEPS)[number];

/** Illustration de chaque étape, dans l'espace à droite de l'assistant. */
const STEP_ICONS: Record<Step, LucideIcon> = {
  library: Images,
  backup: HardDrive,
  folders: FolderTree,
  identity: UserRound,
};

function StepIllustration({ step }: { step: Step }) {
  const Icon = STEP_ICONS[step];
  return (
    <div className="onboarding-illustration" aria-hidden="true">
      <Icon key={step} strokeWidth={1} />
    </div>
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
      <StepIllustration step={step} />
    </div>
  );
}
