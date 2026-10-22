import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: () => Promise.resolve(null),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useDevelop } = await import("../store/develop");
const { LensSection } = await import("./DevelopAdvanced");
const { useLayout } = await import("../store/layout");

function show(rendered: boolean) {
  useLayout.setState({ collapsed: { "develop.lens": false } });
  const setLens = vi.fn();
  useDevelop.setState({
    lensName: "FE 24-70mm F2.8 GM",
    rendered,
    settings: { version: 1 } as never,
    setLens,
  });
  render(<LensSection />);
  return setLens;
}

describe("correction d'objectif", () => {
  afterEach(() => useDevelop.setState({ lensName: null, rendered: false, settings: null }));

  it("activée par défaut sur un RAW", () => {
    show(false);
    expect(screen.getByRole("checkbox", { name: /Corriger l'objectif/ })).toBeChecked();
  });

  it("désactivée par défaut sur un JPEG, et activable", () => {
    const setLens = show(true);
    const box = screen.getByRole("checkbox", { name: /Corriger l'objectif/ });
    expect(box).not.toBeChecked();
    expect(screen.getAllByRole("slider").every((s) => (s as HTMLInputElement).disabled)).toBe(true);
    fireEvent.click(box);
    expect(setLens).toHaveBeenCalledWith("enabled", true);
  });
});
