import { useDialog } from "../store/dialog";

interface ConfirmOptions {
  title: string;
  okLabel: string;
  cancelLabel: string;
}

/** Confirmation d'une action destructive, dans une boîte aux couleurs d'Afterdusk. */
export function confirmWarning(text: string, options: ConfirmOptions): Promise<boolean> {
  return useDialog.getState().show({ kind: "confirm", warning: true, text, ...options });
}
