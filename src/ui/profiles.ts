/**
 * Profils d'interface (cahier des charges §6, V0.5) : ce que chaque profil
 * affiche. Une fonction absente de la table est toujours visible. Masquer
 * ne coupe rien : ce qui est masqué continue de tourner (analyse des ratés,
 * sauvegarde, aperçus, import automatique, réception FTP).
 */
export type Profile = "simple" | "basic" | "complete";

export const PROFILES: Profile[] = ["simple", "basic", "complete"];

const RANK: Record<Profile, number> = { simple: 0, basic: 1, complete: 2 };

/** Profil minimal qui affiche chaque fonction (modules, sections, outils). */
export const FEATURES: Record<string, Profile> = {
  // Modules.
  "module.library": "basic",
  "module.map": "basic",
  "module.gear": "complete",
  // Import : carte ou dossier et album en Simplifiée.
  "import.renaming": "basic",
  "import.metadata": "basic",
  "import.pairs": "basic",
  "import.backup": "complete",
  "settings.autoImport": "complete",
  // Tri : notes, drapeaux, ratés probables en Simplifiée.
  "cull.colors": "basic",
  "cull.filterBar": "basic",
  "cull.bursts": "basic",
  "cull.locate": "basic",
  "cull.collections": "complete",
  "library.collections": "complete",
  "cull.clients": "complete",
  "library.clients": "complete",
  "cull.albumClient": "complete",
  "library.albumClient": "complete",
  // Retouche : lumière, couleur, recadrage, presets en Simplifiée.
  "develop.curve": "basic",
  "develop.hsl": "basic",
  "develop.detail": "basic",
  "develop.lens": "basic",
  "develop.effects": "basic",
  "develop.history": "basic",
  "develop.versions": "complete",
  "develop.snapshots": "complete",
  "develop.spots": "complete",
  "develop.collections": "complete",
  // Livraison : presets, filigrane simple et formats en Simplifiée.
  "deliver.contactSheet": "basic",
  "deliver.gallery": "basic",
  "deliver.designs": "basic",
  "deliver.presetOptions": "complete",
  "deliver.clients": "complete",
  "deliver.collections": "complete",
  "design.systemFonts": "complete",
  // Vidéo.
  "video.clips": "basic",
};

export function allows(profile: Profile, feature: string): boolean {
  const min = FEATURES[feature];
  return min === undefined || RANK[profile] >= RANK[min];
}

/** Sections de panneau d'un écran masquées par le profil. */
export function hiddenSections(profile: Profile, screen: string): string[] {
  return Object.keys(FEATURES).filter(
    (f) => f.startsWith(`${screen}.`) && !allows(profile, f) && SECTION_FEATURES.has(f),
  );
}

/**
 * Fonctions qui sont des sections de panneau (`PanelSection id`) : le lien
 * « Afficher les réglages avancés » les révèle pour la session.
 */
export const SECTION_FEATURES = new Set([
  "import.renaming",
  "import.metadata",
  "import.pairs",
  "import.backup",
  "cull.collections",
  "library.collections",
  "cull.clients",
  "library.clients",
  "cull.albumClient",
  "library.albumClient",
  "develop.curve",
  "develop.hsl",
  "develop.detail",
  "develop.lens",
  "develop.effects",
  "develop.history",
  "develop.versions",
  "develop.snapshots",
  "develop.collections",
  "deliver.collections",
]);

/** Commandes de menu (identifiant) et la fonction qui les porte. */
export const MENU_FEATURES: Record<string, string> = {
  "go-library": "module.library",
  "go-map": "module.map",
  "go-gear": "module.gear",
  "color-red": "cull.colors",
  "color-yellow": "cull.colors",
  "color-green": "cull.colors",
  "color-blue": "cull.colors",
  "color-purple": "cull.colors",
  locate: "cull.locate",
  "time-fix": "cull.locate",
  "stack-toggle": "cull.bursts",
  "stack-top": "cull.bursts",
  "bursts-detect": "cull.bursts",
  "burst-stack": "cull.bursts",
  "burst-unstack": "cull.bursts",
  "trash-jpeg": "import.pairs",
  "trash-raw": "import.pairs",
};
