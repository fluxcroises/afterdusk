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
const { IdentityFields } = await import("./Fields");
const { defaultCopyright } = await import("./presets");

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
});
