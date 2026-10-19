import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Identity } from "../api/core";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { applyChoices } = await import("./onboarding");
const { IdentityFields, TemplateChoice } = await import("./Fields");
const { defaultCopyright } = await import("./presets");

function TemplateHarness() {
  const [folder, setFolder] = useState("{yyyy}/{yyyy}-{mm}-{dd} {album}");
  return (
    <TemplateChoice folderTemplate={folder} renameTemplate={null} onChange={(f) => setFolder(f)} />
  );
}

function Harness() {
  const [identity, setIdentity] = useState<Identity>({ name: "", copyright: "", website: "" });
  return <IdentityFields identity={identity} onChange={setIdentity} />;
}

describe("première ouverture", () => {
  beforeEach(() => {
    invoke
      .mockReset()
      .mockImplementation((cmd) => Promise.resolve(cmd === "library_create" ? { id: 7 } : null));
  });

  it("crée la bibliothèque, puis enregistre rangement et identité", async () => {
    const identity = { name: "Lewis Bedar", copyright: "© 2026 Lewis Bedar", website: "" };
    await applyChoices({
      name: "Photos",
      root: "D:/Photos",
      backup: "E:/Sauvegarde",
      folderTemplate: "{yyyy}/{mm}/{dd}",
      renameTemplate: null,
      identity,
    });
    expect(invoke.mock.calls).toEqual([
      ["library_create", { name: "Photos", root: "D:/Photos", backup: "E:/Sauvegarde" }],
      [
        "library_set_templates",
        { id: 7, folderTemplate: "{yyyy}/{mm}/{dd}", renameTemplate: null },
      ],
      ["identity_set", { identity }],
    ]);
  });

  it("le copyright suit le nom tant qu'il n'est pas personnalisé", () => {
    render(<Harness />);
    const name = screen.getByRole("textbox", { name: "Nom" });
    const copyright = screen.getByRole("textbox", { name: "Copyright" });
    fireEvent.change(name, { target: { value: "Lewis" } });
    expect(copyright).toHaveValue(defaultCopyright("Lewis"));
    fireEvent.change(copyright, { target: { value: "CC BY 4.0" } });
    fireEvent.change(name, { target: { value: "Lewis Bedar" } });
    expect(copyright).toHaveValue("CC BY 4.0");
  });

  it("« Personnalisé » ouvre la saisie à partir du modèle en cours", () => {
    render(<TemplateHarness />);
    fireEvent.click(screen.getByRole("radio", { name: "Personnalisé" }));
    expect(screen.getByRole("radio", { name: "Personnalisé" })).toBeChecked();
    expect(screen.getByRole("radio", { name: /Année \/ date et album/ })).not.toBeChecked();
    const input = screen.getByRole("textbox", { name: "Modèle de dossier" });
    expect(input).toHaveValue("{yyyy}/{yyyy}-{mm}-{dd} {album}");
    // Un modèle saisi qui ressemble à un modèle proposé reste personnalisé.
    fireEvent.change(input, { target: { value: "{yyyy}/{album}" } });
    expect(screen.getByRole("radio", { name: "Personnalisé" })).toBeChecked();
    fireEvent.click(screen.getByRole("radio", { name: /Année \/ mois \/ jour/ }));
    expect(screen.queryByRole("textbox", { name: "Modèle de dossier" })).toBeNull();
  });
});
