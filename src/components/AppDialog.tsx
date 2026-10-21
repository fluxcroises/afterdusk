import { Info, TriangleAlert } from "lucide-react";
import { useEffect, useRef } from "react";
import { useDialog } from "../store/dialog";

/**
 * Questions et messages de l'application, aux couleurs d'Afterdusk plutôt
 * que dans une boîte du système. Une confirmation destructive met « Annuler »
 * en avant ; Échap annule.
 */
export function AppDialog() {
  const current = useDialog((s) => s.current);
  const answer = useDialog((s) => s.answer);
  const safe = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!current) return;
    safe.current?.focus();
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      answer(current.kind === "message");
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [current, answer]);

  if (!current) return null;
  const Icon = current.warning ? TriangleAlert : Info;
  const confirm = current.kind === "confirm";
  return (
    <div className="modal-backdrop">
      <div
        className="copy-dialog app-dialog"
        role={current.warning ? "alertdialog" : "dialog"}
        aria-modal="true"
        aria-labelledby="app-dialog-title"
        aria-describedby="app-dialog-text"
        data-warning={current.warning || undefined}
      >
        <div className="app-dialog-head">
          <Icon className="app-dialog-icon" aria-hidden="true" strokeWidth={1.75} />
          <h2 id="app-dialog-title" className="copy-title">
            {current.title}
          </h2>
        </div>
        <p id="app-dialog-text" className="app-dialog-text">
          {current.text}
        </p>
        <div className="develop-crop-actions">
          <span className="toolbar-spacer" />
          {confirm && (
            <button
              ref={current.warning ? safe : undefined}
              type="button"
              className="button button-small"
              onClick={() => answer(false)}
            >
              {current.cancelLabel}
            </button>
          )}
          <button
            ref={confirm && current.warning ? undefined : safe}
            type="button"
            className={
              current.warning
                ? "button button-small button-danger"
                : "button button-small button-primary"
            }
            onClick={() => answer(true)}
          >
            {current.okLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
