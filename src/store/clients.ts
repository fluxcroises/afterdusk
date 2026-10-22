import { invoke } from "@tauri-apps/api/core";
import type { TFunction } from "i18next";
import { create } from "zustand";
import { toCommandError } from "../api/core";
import { useLibrary } from "./library";

/** Miroir de `library::clients::Client`. */
export interface Client {
  id: number;
  name: string;
  email: string | null;
  phone: string | null;
  notes: string | null;
  albumCount: number;
}

export interface ClientInput {
  name: string;
  email: string;
  phone: string;
  notes: string;
}

/** Album d'un client, pour sa fiche. */
export interface ClientAlbum {
  id: number;
  libraryId: number;
  libraryName: string;
  name: string;
  startDate: string | null;
  sessionTypeId: number | null;
}

/** Type de séance : `key` pour un type d'origine, `name` pour un type ajouté ou renommé. */
export interface SessionType {
  id: number;
  key: string | null;
  name: string | null;
}

/** Nom affiché d'un type de séance. */
export function sessionLabel(t: TFunction, type: SessionType | undefined): string {
  if (!type) return "";
  return type.name ?? t(`sessions.kinds.${type.key}`, { defaultValue: type.key ?? "" });
}

/** Fiche ouverte : un client existant, ou une nouvelle fiche (à rattacher à un album). */
export type ClientSheet = { id: number } | { id: null; attachTo: number | null };

interface ClientsState {
  clients: Client[];
  sessionTypes: SessionType[];
  sheet: ClientSheet | null;
  load: () => Promise<void>;
  openSheet: (sheet: ClientSheet) => void;
  closeSheet: () => void;
  /** Enregistre la fiche ouverte ; une nouvelle fiche se rattache à l'album demandé. */
  save: (input: ClientInput) => Promise<boolean>;
  remove: (id: number) => Promise<void>;
  albumsOf: (id: number) => Promise<ClientAlbum[]>;
  setAlbum: (
    albumId: number,
    clientId: number | null,
    sessionTypeId: number | null,
  ) => Promise<void>;
  createType: (name: string) => Promise<void>;
  renameType: (id: number, name: string) => Promise<void>;
  removeType: (id: number) => Promise<void>;
}

const payload = (input: ClientInput) => ({
  name: input.name.trim(),
  email: input.email.trim() || null,
  phone: input.phone.trim() || null,
  notes: input.notes.trim() || null,
});

export const useClients = create<ClientsState>()((set, get) => {
  async function guard(run: () => Promise<void>): Promise<boolean> {
    try {
      await run();
      return true;
    } catch (e) {
      useLibrary.setState({ error: toCommandError(e) });
      return false;
    }
  }

  /** Les albums affichent leur client : on les relit après chaque changement. */
  async function refresh() {
    await get().load();
    await useLibrary.getState().reloadAlbums();
  }

  return {
    clients: [],
    sessionTypes: [],
    sheet: null,

    load: async () => {
      await guard(async () => {
        const [clients, sessionTypes] = await Promise.all([
          invoke<Client[] | null>("clients_list"),
          invoke<SessionType[] | null>("session_types_list"),
        ]);
        set({ clients: clients ?? [], sessionTypes: sessionTypes ?? [] });
      });
    },

    openSheet: (sheet) => set({ sheet }),
    closeSheet: () => set({ sheet: null }),

    save: (input) =>
      guard(async () => {
        const sheet = get().sheet;
        if (!sheet) return;
        if (sheet.id !== null) {
          await invoke("client_update", { id: sheet.id, client: payload(input) });
        } else {
          const id = await invoke<number>("client_create", { client: payload(input) });
          if (sheet.attachTo !== null) {
            const album = useLibrary.getState().albums.find((a) => a.id === sheet.attachTo);
            await invoke("album_set_client", {
              albumId: sheet.attachTo,
              clientId: id,
              sessionTypeId: album?.sessionTypeId ?? null,
            });
          }
        }
        set({ sheet: null });
        await refresh();
      }),

    remove: async (id) => {
      await guard(async () => {
        await invoke("client_delete", { id });
        set({ sheet: null });
        const library = useLibrary.getState();
        // Le filtre sur ce client ne montrerait plus rien.
        if (library.filter.clientId === id) await library.setFilter({ clientId: undefined });
        await refresh();
      });
    },

    albumsOf: async (id) => {
      try {
        return (await invoke<ClientAlbum[] | null>("client_albums", { id })) ?? [];
      } catch (e) {
        useLibrary.setState({ error: toCommandError(e) });
        return [];
      }
    },

    setAlbum: async (albumId, clientId, sessionTypeId) => {
      await guard(async () => {
        await invoke("album_set_client", { albumId, clientId, sessionTypeId });
        await refresh();
      });
    },

    createType: async (name) => {
      await guard(async () => {
        await invoke<number>("session_type_create", { name });
        await get().load();
      });
    },

    renameType: async (id, name) => {
      await guard(async () => {
        await invoke("session_type_rename", { id, name });
        await get().load();
      });
    },

    removeType: async (id) => {
      await guard(async () => {
        await invoke("session_type_delete", { id });
        const library = useLibrary.getState();
        if (library.filter.sessionTypeId === id) {
          await library.setFilter({ sessionTypeId: undefined });
        }
        await refresh();
      });
    },
  };
});
