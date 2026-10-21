import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export type MaintenanceKind = "sensor" | "service" | "repair" | "firmware" | "other";

export const MAINTENANCE_KINDS: MaintenanceKind[] = [
  "sensor",
  "service",
  "repair",
  "firmware",
  "other",
];

/** Intervalles de rappel proposés, en mois. */
export const REMINDER_MONTHS = [3, 6, 12, 24];

/** Miroir de `gear::maintenance::MaintenanceInput`. */
export interface MaintenanceInput {
  /** `AAAA-MM-JJ`. */
  date: string;
  kind: MaintenanceKind;
  costCents: number | null;
  currency: string | null;
  notes: string | null;
  reminderMonths: number | null;
}

export interface MaintenanceEntry extends MaintenanceInput {
  id: number;
  gearId: number;
}

export interface Reminder {
  gearId: number;
  gearName: string;
  kind: MaintenanceKind;
  lastDate: string;
  dueDate: string;
  due: boolean;
}

interface RemindersState {
  /** Rappels de tout le matériel visible, les plus proches d'abord. */
  reminders: Reminder[];
  load: () => Promise<void>;
}

/** Rappels d'entretien de tout le matériel : onglet Matériel, liste, accueil. */
export const useReminders = create<RemindersState>()((set) => ({
  reminders: [],
  load: async () => {
    try {
      const reminders = await invoke<Reminder[] | null>("maintenance_reminders", {
        gearId: null,
      });
      set({ reminders: reminders ?? [] });
    } catch {
      // Sans rappels, l'écran reste utilisable ; l'erreur ressortira au Matériel.
      set({ reminders: [] });
    }
  },
}));

export function dueCount(reminders: Reminder[]): number {
  return reminders.filter((r) => r.due).length;
}
