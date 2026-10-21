import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));
const open = vi.fn<() => Promise<string[] | null>>();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: () => open() }));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useLocate } = await import("../store/locate");
const { LocateDialog } = await import("./LocateDialog");

describe("Photo > Localiser…", () => {
  beforeEach(() => {
    invoke
      .mockReset()
      .mockImplementation((cmd) =>
        Promise.resolve(
          cmd === "geo_clock"
            ? { writtenMinutes: 120, withOffset: 2, total: 3 }
            : { located: 2, kept: 1, unmatched: 0, noTime: 0, unusableSources: 0 },
        ),
      );
    open.mockReset().mockResolvedValue(["D:/Traces/montre.gpx"]);
  });

  it("propose le fuseau du boîtier, localise et donne le bilan", async () => {
    await act(() => useLocate.getState().open([4, 5, 6]));
    render(<LocateDialog />);
    expect(screen.getByText("Localiser 3 photos")).toBeInTheDocument();
    expect(
      screen.getByLabelText("Fuseau écrit par le boîtier : UTC +02:00 (2 sur 3)"),
    ).toBeChecked();
    const submit = screen.getByRole("button", { name: "Localiser" });
    expect(submit).toBeDisabled();
    await act(async () => fireEvent.click(screen.getByText("Choisir des traces…")));
    expect(screen.getByText("montre.gpx")).toBeInTheDocument();
    await act(async () => fireEvent.click(submit));
    expect(invoke).toHaveBeenCalledWith("geo_locate_gpx", {
      ids: [4, 5, 6],
      files: ["D:/Traces/montre.gpx"],
      offsetMinutes: null,
      maxGapMinutes: 10,
      replace: false,
    });
    expect(screen.getByRole("status")).toHaveTextContent("2 photos localisées · 1 déjà localisée");
  });

  it("photos de téléphone avec un décalage saisi", async () => {
    await act(() => useLocate.getState().open([7]));
    render(<LocateDialog />);
    fireEvent.click(screen.getByRole("radio", { name: "Photos de téléphone" }));
    await act(async () => fireEvent.click(screen.getByText("Choisir des photos…")));
    fireEvent.change(screen.getByLabelText("Décalage sur UTC, en heures"), {
      target: { value: "-5" },
    });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Localiser" })));
    expect(invoke).toHaveBeenCalledWith("geo_locate_phone", {
      ids: [7],
      files: ["D:/Traces/montre.gpx"],
      offsetMinutes: -300,
      windowMinutes: 30,
      replace: false,
    });
  });
});
