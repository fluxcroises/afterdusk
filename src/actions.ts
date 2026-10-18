import { open } from "@tauri-apps/plugin-dialog";
import i18n from "./i18n";
import { useImport } from "./store/import";
import { useNavigation } from "./store/navigation";

/** Sélecteur de dossier natif ; `null` si l'utilisateur annule. */
export async function pickFolder(title: string): Promise<string | null> {
  const result = await open({ directory: true, multiple: false, title });
  return typeof result === "string" ? result : null;
}

/** Fichier > Importer depuis un dossier… : ouvre Importer puis analyse le dossier choisi. */
export async function importFromFolder(): Promise<void> {
  useNavigation.getState().goTo("import");
  const path = await pickFolder(i18n.t("import.chooseFolder"));
  if (path) await useImport.getState().scanSource(path);
}
