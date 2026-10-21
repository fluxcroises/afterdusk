import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useImport } = await import("../store/import");
const { useLocate } = await import("../store/locate");
const { ImportStep } = await import("./ImportStep");

describe("fin d'import : photos sans lieu", () => {
  let unlocated = [11, 12, 13];
  beforeEach(() => {
    unlocated = [11, 12, 13];
    invoke.mockReset().mockImplementation((cmd) => {
      if (cmd === "geo_unlocated") return Promise.resolve(unlocated);
      if (cmd === "geo_clock")
        return Promise.resolve({ writtenMinutes: 60, mixed: false, withOffset: 3, total: 3 });
      if (cmd === "geo_locate_manual") {
        unlocated = [];
        return Promise.resolve({
          located: 3,
          kept: 0,
          unmatched: 0,
          noTime: 0,
          unusableSources: 0,
        });
      }
      return Promise.resolve([]);
    });
    useImport.setState({ source: null, running: false, lastResult: null, unlocated: [] });
  });

  it("propose de localiser les photos sans GPS, puis plus rien une fois localisées", async () => {
    render(<ImportStep />);
    await act(async () =>
      useImport.getState().onFinished({ importId: 7, outcome: "done", error: null }),
    );
    expect(invoke).toHaveBeenCalledWith("geo_unlocated", { importId: 7 });
    const offer = await screen.findByRole("button", {
      name: "Localiser les 3 photos sans lieu…",
    });
    await act(async () => fireEvent.click(offer));
    expect(useLocate.getState().ids).toEqual([11, 12, 13]);
    await act(() =>
      useLocate.getState().run({
        source: "manual",
        files: [],
        place: { latitude: 49.29, longitude: -0.11 },
        offsetMinutes: null,
        minutes: 0,
        replace: false,
      }),
    );
    expect(useImport.getState().unlocated).toEqual([]);
    expect(screen.queryByText(/sans lieu…/)).not.toBeInTheDocument();
    useLocate.getState().close();
  });

  it("rien à proposer quand toutes les photos ont un lieu", async () => {
    unlocated = [];
    render(<ImportStep />);
    await act(async () =>
      useImport.getState().onFinished({ importId: 8, outcome: "done", error: null }),
    );
    expect(screen.queryByText(/sans lieu…/)).not.toBeInTheDocument();
  });
});
