import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useDeliver } = await import("../store/deliver");
const { ExportPresetsSection } = await import("./DeliverPanels");

const preset = {
  id: "web",
  name: null,
  output: { kind: "heif" as const, quality: 80 },
  resize: { mode: "longSide" as const, value: 2048 },
  color: "srgb" as const,
  sharpen: "screen" as const,
  metadata: "noGps" as const,
  watermark: false,
  naming: "{name}",
};

function show(heifAvailable: boolean) {
  useDeliver.setState({
    settings: {
      version: 1,
      presets: [preset],
      watermark: null,
      destination: null,
      openFolder: false,
    },
    editing: "web",
    heifAvailable,
  });
  render(<ExportPresetsSection />);
}

describe("format HEIF", () => {
  afterEach(() => useDeliver.setState({ settings: null, editing: null, heifAvailable: false }));

  it("se choisit quand le système sait l'écrire", () => {
    show(true);
    expect(screen.getByText("HEIF 80 · 2048 px · sRGB")).toBeInTheDocument();
    const option = screen.getByRole<HTMLOptionElement>("option", { name: "HEIF" });
    expect(option.disabled).toBe(false);
    expect(screen.getByText("Qualité")).toBeInTheDocument();
  });

  it("est grisé, avec la raison, sans encodeur", () => {
    show(false);
    const option = screen.getByRole<HTMLOptionElement>("option", { name: "HEIF (indisponible)" });
    expect(option.disabled).toBe(true);
    expect(screen.getByText(/Encodeur HEIF absent/)).toBeInTheDocument();
  });
});
