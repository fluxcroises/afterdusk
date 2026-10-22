import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { create } from "zustand";
import i18n from "../i18n";
import { type WindowLabel, openMain } from "../windows";
import { useImport } from "./import";
import { useNavigation } from "./navigation";

/** Carte ou boîtier détecté par le cœur (`devices.rs`). */
export interface Device {
  root: string;
  volume: string;
  camera: string | null;
}

export type InsertBehavior = "propose" | "open" | "nothing";

/** Issue de l'insertion d'une carte, miroir de `commands::import::AutoCard`. */
export type AutoCard =
  | { kind: "unknown" }
  | { kind: "busy" }
  | { kind: "nothingNew"; camera: string }
  | { kind: "started"; camera: string; importId: number; files: number };

/** Import automatique en cours ou terminé, affiché dans le bandeau. */
export type AutoImport =
  | { kind: "nothingNew"; camera: string }
  | { kind: "started"; camera: string; importId: number; files: number };

export const devicesApi = {
  list: () => invoke<Device[]>("devices_list"),
  behavior: () => invoke<InsertBehavior>("insert_behavior_get"),
  setBehavior: (behavior: InsertBehavior) => invoke<null>("insert_behavior_set", { behavior }),
  attention: (label: WindowLabel) => invoke<null>("request_attention", { label }),
  autoCard: (root: string, sourceLabel: string) =>
    invoke<AutoCard>("auto_import_card", { root, sourceLabel }),
};

/** « Carte FUJI (E:) », « Carte (E:) ». */
export function deviceLabel(d: Device): string {
  return d.camera
    ? i18n.t("devices.cardOf", { camera: d.camera, volume: d.volume })
    : i18n.t("devices.card", { volume: d.volume });
}

/** Cette fenêtre est-elle celle qui répond à l'insertion ? L'accueil s'il est affiché, sinon la principale. */
async function answers(label: WindowLabel): Promise<boolean> {
  if (!isTauri()) return true;
  if (label === "welcome") return getCurrentWindow().isVisible();
  if (label !== "main") return false;
  const welcome = await WebviewWindow.getByLabel("welcome");
  return !(await welcome?.isVisible());
}

interface DevicesState {
  devices: Device[];
  /** Carte insérée dont l'import est proposé (bandeau). */
  pending: Device | null;
  /** Carte d'un boîtier connu, importée seule (bandeau). */
  auto: AutoImport | null;
  dismissAuto: () => void;
  behavior: InsertBehavior;
  load: () => Promise<void>;
  attached: (device: Device, label: WindowLabel) => Promise<void>;
  detached: (device: Device) => void;
  /** Ouvre Importer sur la carte, depuis l'accueil ou la fenêtre principale. */
  openImport: (device: Device, label: WindowLabel) => Promise<void>;
  dismiss: () => void;
  /** La carte est ouverte dans Importer : son bandeau n'a plus lieu d'être. */
  handled: (root: string) => void;
  setBehavior: (behavior: InsertBehavior) => Promise<void>;
}

export const useDevices = create<DevicesState>((set, get) => ({
  devices: [],
  pending: null,
  auto: null,
  dismissAuto: () => set({ auto: null }),
  behavior: "propose",

  load: async () => {
    try {
      const [devices, behavior] = await Promise.all([devicesApi.list(), devicesApi.behavior()]);
      set({ devices, behavior });
    } catch {
      // Sans surveillance des supports, l'import par dossier reste possible.
    }
  },

  attached: async (device, label) => {
    set((s) => ({
      devices: [...s.devices.filter((d) => d.root !== device.root), device],
    }));
    // Le réglage a pu changer dans l'autre fenêtre.
    const behavior = await devicesApi.behavior().catch(() => get().behavior);
    set({ behavior });
    if (behavior === "nothing") return;
    const importing = useImport.getState().running;
    // Carte d'un boîtier connu : l'import part seul, depuis une seule fenêtre.
    if (!importing && (await answers(label))) {
      const card = await devicesApi
        .autoCard(device.root, deviceLabel(device))
        .catch((): AutoCard => ({ kind: "unknown" }));
      if (card?.kind === "started" || card?.kind === "nothingNew") {
        if (card.kind === "started") {
          useImport.setState({ running: true, progress: null, lastResult: null, error: null });
        }
        set({ auto: card, pending: null });
        if (isTauri() && !document.hasFocus()) {
          void devicesApi.attention(label).catch(() => undefined);
        }
        return;
      }
    }
    if (behavior === "open" && !importing) {
      if (await answers(label)) await get().openImport(device, label);
      return;
    }
    set({ pending: device });
    if (isTauri() && (await answers(label)) && !document.hasFocus()) {
      void devicesApi.attention(label).catch(() => undefined);
    }
  },

  detached: (device) =>
    set((s) => ({
      devices: s.devices.filter((d) => d.root !== device.root),
      pending: s.pending?.root === device.root ? null : s.pending,
    })),

  openImport: async (device, label) => {
    set({ pending: null });
    if (label === "welcome") {
      await openMain({ screen: "import", scanPath: device.root });
      return;
    }
    useNavigation.getState().goTo("import");
    await useImport.getState().scanSource(device.root);
  },

  dismiss: () => set({ pending: null }),

  handled: (root) => set((s) => (s.pending?.root === root ? { pending: null } : s)),

  setBehavior: async (behavior) => {
    set({ behavior });
    await devicesApi.setBehavior(behavior).catch(() => undefined);
  },
}));
