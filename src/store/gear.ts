import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { type CommandError, toCommandError } from "../api/core";

export type GearKind =
  "body" | "lens" | "flash" | "filter" | "tripod" | "bag" | "battery" | "other";

export const GEAR_KINDS: GearKind[] = [
  "body",
  "lens",
  "flash",
  "filter",
  "tripod",
  "bag",
  "battery",
  "other",
];

/** Silhouettes dessinées pour Afterdusk. */
export type GearStyle = "mirrorless" | "dslr" | "compact" | "prime" | "zoom";

/** Ligne de l'inventaire, miroir de `gear::GearItem`. */
export interface GearItem {
  id: number;
  kind: GearKind;
  make: string;
  model: string;
  displayName: string;
  style: GearStyle | null;
  serial: string | null;
  autoDetected: boolean;
  hidden: boolean;
  confirmed: boolean;
  photoMediaId: number | null;
  hasPhotoFile: boolean;
  photoCount: number;
  firstUsed: string | null;
  lastUsed: string | null;
}

export interface GearRecord extends GearItem {
  name: string | null;
  purchaseDate: string | null;
  purchasePlace: string | null;
  priceCents: number | null;
  currency: string | null;
  condition: string | null;
  warrantyUntil: string | null;
  notes: string | null;
  aliases: string[];
}

/** Champs d'une fiche, tels que le formulaire les envoie. */
export type GearUpdate = Partial<
  Pick<
    GearRecord,
    | "kind"
    | "make"
    | "model"
    | "name"
    | "style"
    | "serial"
    | "purchaseDate"
    | "purchasePlace"
    | "priceCents"
    | "currency"
    | "condition"
    | "warrantyUntil"
    | "notes"
  >
>;

interface GearState {
  items: GearItem[];
  showHidden: boolean;
  selectedId: number | null;
  record: GearRecord | null;
  /** Image déposée sur la fiche, en adresse locale (`blob:`). */
  photoUrl: string | null;
  error: CommandError | null;
  load: () => Promise<void>;
  setShowHidden: (show: boolean) => Promise<void>;
  select: (id: number | null) => Promise<void>;
  save: (update: GearUpdate) => Promise<void>;
  create: (kind: GearKind) => Promise<void>;
  remove: () => Promise<void>;
  setHidden: (hidden: boolean) => Promise<void>;
  setConfirmed: (confirmed: boolean) => Promise<void>;
  merge: (target: number) => Promise<void>;
  setPhotoMedia: (mediaId: number | null) => Promise<void>;
  setPhotoFile: (path: string) => Promise<void>;
}

export const useGear = create<GearState>()((set, get) => {
  async function guard(run: () => Promise<void>) {
    try {
      await run();
    } catch (e) {
      set({ error: toCommandError(e) });
    }
  }

  async function loadPhoto(record: GearRecord | null) {
    const previous = get().photoUrl;
    if (previous) URL.revokeObjectURL(previous);
    if (!record?.hasPhotoFile) {
      set({ photoUrl: null });
      return;
    }
    const bytes = await invoke<ArrayBuffer>("gear_photo_file", { id: record.id });
    set({ photoUrl: bytes.byteLength > 0 ? URL.createObjectURL(new Blob([bytes])) : null });
  }

  async function refresh(id: number | null) {
    const items = await invoke<GearItem[]>("gear_list", { includeHidden: get().showHidden });
    const keep = id !== null && items.some((g) => g.id === id) ? id : (items[0]?.id ?? null);
    const record = keep === null ? null : await invoke<GearRecord>("gear_get", { id: keep });
    set({ items, selectedId: keep, record });
    await loadPhoto(record);
  }

  return {
    items: [],
    showHidden: false,
    selectedId: null,
    record: null,
    photoUrl: null,
    error: null,

    load: () => guard(() => refresh(get().selectedId)),

    setShowHidden: (showHidden) =>
      guard(async () => {
        set({ showHidden });
        await refresh(get().selectedId);
      }),

    select: (id) =>
      guard(async () => {
        set({ selectedId: id });
        const record = id === null ? null : await invoke<GearRecord>("gear_get", { id });
        set({ record });
        await loadPhoto(record);
      }),

    save: (update) =>
      guard(async () => {
        const id = get().selectedId;
        if (id === null) return;
        const current = get().record;
        await invoke<GearRecord>("gear_update", { id, update: { ...current, ...update } });
        await refresh(id);
      }),

    create: (kind) =>
      guard(async () => {
        const id = await invoke<number>("gear_create", { kind, make: "", model: "" });
        await refresh(id);
      }),

    remove: () =>
      guard(async () => {
        const id = get().selectedId;
        if (id === null) return;
        await invoke("gear_delete", { id });
        await refresh(null);
      }),

    setHidden: (hidden) =>
      guard(async () => {
        const id = get().selectedId;
        if (id === null) return;
        await invoke("gear_set_hidden", { id, hidden });
        await refresh(id);
      }),

    setConfirmed: (confirmed) =>
      guard(async () => {
        const id = get().selectedId;
        if (id === null) return;
        await invoke("gear_set_confirmed", { id, confirmed });
        await refresh(id);
      }),

    merge: (target) =>
      guard(async () => {
        const source = get().selectedId;
        if (source === null) return;
        await invoke("gear_merge", { source, target });
        await refresh(target);
      }),

    setPhotoMedia: (mediaId) =>
      guard(async () => {
        const id = get().selectedId;
        if (id === null) return;
        await invoke("gear_set_photo_media", { id, mediaId });
        await refresh(id);
      }),

    setPhotoFile: (path) =>
      guard(async () => {
        const id = get().selectedId;
        if (id === null) return;
        await invoke("gear_set_photo_file", { id, path });
        await refresh(id);
      }),
  };
});
