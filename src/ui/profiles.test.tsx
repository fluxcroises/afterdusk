import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: () => Promise.resolve(null),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { allows, hiddenSections, FEATURES, SECTION_FEATURES, MENU_FEATURES } =
  await import("./profiles");
const { useProfile } = await import("../store/profile");
const { useNavigation } = await import("../store/navigation");
const { PanelSection } = await import("../components/PanelSection");
const { ScreenLayout } = await import("../components/ScreenLayout");
const { ModulePicker } = await import("../components/ModulePicker");
const { buildMenuItems } = await import("../menu");
const { GridToolbar } = await import("../components/GridToolbar");
const { handleShortcut } = await import("../desktop");
const { useLibrary } = await import("../store/library");

type Entry = { id?: string; item?: string; items?: Entry[] };
function ids(items: Entry[]): string[] {
  return items.flatMap((e) => (e.items ? ids(e.items) : e.id ? [e.id] : []));
}
function separatorsOk(items: Entry[]): boolean {
  return items.every((e, i) => {
    if (e.items) return separatorsOk(e.items);
    if (e.item !== "Separator") return true;
    const prev = items[i - 1];
    return i > 0 && i < items.length - 1 && prev?.item !== "Separator";
  });
}

describe("profils d'interface", () => {
  afterEach(() => {
    useProfile.setState({ profile: "complete", revealed: false });
    useNavigation.setState({ screen: "import" });
  });

  it("chaque profil montre au moins ce que montre le précédent", () => {
    for (const feature of Object.keys(FEATURES)) {
      if (allows("simple", feature)) expect(allows("basic", feature)).toBe(true);
      if (allows("basic", feature)) expect(allows("complete", feature)).toBe(true);
      expect(allows("complete", feature)).toBe(true);
    }
    // Une fonction absente de la table reste visible partout.
    expect(allows("simple", "develop.light")).toBe(true);
    expect(allows("simple", "develop.curve")).toBe(false);
    expect(allows("basic", "develop.curve")).toBe(true);
    expect(allows("basic", "develop.spots")).toBe(false);
    // Répartition des fonctions de la V0.5 (choix de l'utilisateur).
    expect(allows("simple", "deliver.templates")).toBe(true);
    expect(allows("simple", "deliver.designs")).toBe(false);
    expect(allows("basic", "video.clips")).toBe(true);
    expect(allows("basic", "design.systemFonts")).toBe(false);
    for (const f of SECTION_FEATURES) expect(FEATURES[f]).toBeDefined();
    for (const f of Object.values(MENU_FEATURES)) expect(FEATURES[f]).toBeDefined();
  });

  it("sections masquées, puis montrées par « Afficher les réglages avancés »", () => {
    useProfile.setState({ profile: "simple" });
    useNavigation.setState({ screen: "develop" });
    expect(hiddenSections("simple", "develop")).toContain("develop.hsl");
    render(
      <ScreenLayout
        right={
          <>
            <PanelSection id="develop.light" title="Lumière">
              <p>Exposition</p>
            </PanelSection>
            <PanelSection id="develop.hsl" title="TSL">
              <p>Teinte</p>
            </PanelSection>
          </>
        }
      >
        <div />
      </ScreenLayout>,
    );
    expect(screen.getByText("Lumière")).toBeInTheDocument();
    expect(screen.queryByText("TSL")).not.toBeInTheDocument();
    const count = hiddenSections("simple", "develop").length;
    fireEvent.click(
      screen.getByRole("button", { name: `Afficher les ${count} réglages avancés masqués` }),
    );
    expect(screen.getByText("TSL")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Masquer les réglages avancés" }));
    expect(screen.queryByText("TSL")).not.toBeInTheDocument();
  });

  it("en interface complète, rien n'est masqué et aucun lien n'apparaît", () => {
    useNavigation.setState({ screen: "develop" });
    render(
      <ScreenLayout
        right={
          <PanelSection id="develop.spots" title="Correcteur">
            <p />
          </PanelSection>
        }
      >
        <div />
      </ScreenLayout>,
    );
    expect(screen.getByText("Correcteur")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /réglages avancés/ })).toBeNull();
  });

  it("modules selon le profil", () => {
    useProfile.setState({ profile: "simple" });
    const { rerender } = render(<ModulePicker />);
    const names = () =>
      screen.getAllByRole("button").map((b) => b.textContent?.replace(/\d+$/, ""));
    expect(names()).toEqual(["Importer", "Trier", "Retoucher", "Livrer"]);
    act(() => useProfile.setState({ profile: "basic" }));
    rerender(<ModulePicker />);
    expect(names()).toEqual(["Importer", "Trier", "Retoucher", "Livrer", "Bibliothèque", "Carte"]);
  });

  it("menus : commandes des fonctions masquées retirées, séparateurs propres", () => {
    useProfile.setState({ profile: "simple" });
    const simple = buildMenuItems(i18n.t) as Entry[];
    const shown = ids(simple);
    expect(shown).not.toContain("go-gear");
    expect(shown).not.toContain("go-library");
    expect(shown).not.toContain("color-red");
    expect(shown).not.toContain("bursts-detect");
    expect(shown).toContain("go-cull");
    expect(shown).toContain("flag-pick");
    expect(shown).toContain("profile-basic");
    expect(separatorsOk(simple)).toBe(true);
    useProfile.setState({ profile: "complete" });
    const all = ids(buildMenuItems(i18n.t) as Entry[]);
    expect(all).toContain("go-gear");
    expect(all).toContain("color-red");
  });

  it("raccourcis à une touche des fonctions masquées sans effet", () => {
    const toggleColor = vi.fn(() => Promise.resolve());
    useLibrary.setState({ selectedId: 1, toggleColor, trash: false });
    useNavigation.setState({ screen: "cull" });
    useProfile.setState({ profile: "simple" });
    expect(handleShortcut(new KeyboardEvent("keydown", { key: "6", code: "Digit6" }))).toBe(false);
    expect(toggleColor).not.toHaveBeenCalled();
    useProfile.setState({ profile: "basic" });
    expect(handleShortcut(new KeyboardEvent("keydown", { key: "6", code: "Digit6" }))).toBe(true);
    expect(toggleColor).toHaveBeenCalledWith("red");
  });

  it("clients et pastilles de couleur suivent le profil sur tous les écrans", () => {
    // Le panneau de catalogue de chaque écran : clients en Complète seulement.
    expect(allows("basic", "develop.clients")).toBe(false);
    expect(allows("basic", "deliver.collections")).toBe(false);
    expect(hiddenSections("simple", "develop")).toContain("develop.clients");
    expect(allows("complete", "map.albumClient")).toBe(true);
    useProfile.setState({ profile: "simple" });
    const { rerender } = render(<GridToolbar count={3} culling />);
    expect(screen.queryByRole("group", { name: "Étiquette de couleur" })).toBeNull();
    act(() => useProfile.setState({ profile: "basic" }));
    rerender(<GridToolbar count={3} culling />);
    expect(screen.getByRole("group", { name: "Étiquette de couleur" })).toBeInTheDocument();
  });
});
