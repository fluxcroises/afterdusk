import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));

const { invoke } = await import("@tauri-apps/api/core");
const { useProfile, watchProfile } = await import("./profile");
const mocked = vi.mocked(invoke);

describe("profil au lancement", () => {
  afterEach(() => {
    vi.useRealTimers();
    useProfile.setState({ profile: "complete", error: null });
  });

  it("relu tant que le cœur n'est pas prêt, au lieu de rester complet", async () => {
    vi.useFakeTimers();
    let calls = 0;
    mocked.mockImplementation((cmd: string) => {
      if (cmd !== "interface_profile_get") return Promise.resolve(null);
      calls += 1;
      return calls < 3 ? Promise.reject(new Error("state not managed")) : Promise.resolve("basic");
    });
    const done = watchProfile();
    await vi.runAllTimersAsync();
    await done;
    expect(calls).toBe(3);
    expect(useProfile.getState()).toMatchObject({ profile: "basic", error: null });
  });
});
