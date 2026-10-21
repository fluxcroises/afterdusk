import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useTimeFix } = await import("../store/timefix");
const { TimeFixDialog } = await import("./TimeFixDialog");

const report = (groups: number) => ({
  groups:
    groups > 0
      ? [
          {
            city: "New York City",
            cameraMinutes: 60,
            localMinutes: -300,
            shiftMinutes: -360,
            count: 12,
          },
        ]
      : [],
  correct: 2,
  noPlace: 1,
  noZone: 0,
  noTime: 0,
});

describe("Photo > Corriger l'heure…", () => {
  let corrected = false;
  beforeEach(() => {
    corrected = false;
    invoke.mockReset().mockImplementation((cmd) => {
      if (cmd === "geo_clock")
        return Promise.resolve({ writtenMinutes: 60, mixed: false, withOffset: 15, total: 15 });
      if (cmd === "geo_time_correct") {
        corrected = true;
        return Promise.resolve(12);
      }
      if (cmd === "geo_time_proposals") return Promise.resolve(report(corrected ? 0 : 1));
      return Promise.resolve(null);
    });
  });

  it("montre l'écart par lieu, corrige, puis plus rien à proposer", async () => {
    await act(() => useTimeFix.getState().open([1, 2, 3]));
    render(<TimeFixDialog />);
    expect(
      screen.getByText(
        "New York City : horloge UTC +01:00, heure locale UTC -05:00, −6 h (12 photos)",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("2 déjà à l'heure locale · 1 sans lieu")).toBeInTheDocument();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Corriger" })));
    expect(invoke).toHaveBeenCalledWith("geo_time_correct", {
      ids: [1, 2, 3],
      offsetMinutes: null,
    });
    expect(screen.getByRole("status")).toHaveTextContent("12 photos corrigées");
    expect(screen.getByRole("button", { name: "Corriger" })).toBeDisabled();
  });

  it("sans fuseau écrit, l'horloge saisie sert au calcul", async () => {
    invoke.mockImplementation((cmd) =>
      Promise.resolve(
        cmd === "geo_clock"
          ? { writtenMinutes: null, mixed: false, withOffset: 0, total: 1 }
          : report(1),
      ),
    );
    await act(() => useTimeFix.getState().open([4]));
    render(<TimeFixDialog />);
    const call = invoke.mock.calls.find(([cmd]) => cmd === "geo_time_proposals");
    expect(call?.[1]).toEqual({ ids: [4], offsetMinutes: -new Date().getTimezoneOffset() });
    fireEvent.change(screen.getByLabelText("Décalage sur UTC, en heures"), {
      target: { value: "9" },
    });
    expect(invoke).toHaveBeenLastCalledWith("geo_time_proposals", { ids: [4], offsetMinutes: 540 });
  });
});
