import { create } from "zustand";

/** Une question ou un message, affiché dans la fenêtre aux couleurs d'Afterdusk. */
export interface DialogRequest {
  kind: "confirm" | "message";
  title: string;
  text: string;
  okLabel: string;
  /** Confirmations seulement. */
  cancelLabel?: string;
  /** Action qui détruit ou retire quelque chose : icône d'avertissement. */
  warning?: boolean;
}

interface Pending extends DialogRequest {
  resolve: (ok: boolean) => void;
}

interface DialogState {
  current: Pending | null;
  queue: Pending[];
  /** Affiche la boîte ; `true` si l'utilisateur valide. */
  show: (request: DialogRequest) => Promise<boolean>;
  answer: (ok: boolean) => void;
}

/** Boîtes de dialogue de l'application, une à la fois, dans l'ordre des demandes. */
export const useDialog = create<DialogState>()((set, get) => ({
  current: null,
  queue: [],

  show: (request) =>
    new Promise<boolean>((resolve) => {
      const pending = { ...request, resolve };
      if (get().current) set((s) => ({ queue: [...s.queue, pending] }));
      else set({ current: pending });
    }),

  answer: (ok) => {
    const { current, queue } = get();
    if (!current) return;
    const [next = null, ...rest] = queue;
    set({ current: next, queue: rest });
    current.resolve(ok);
  },
}));
