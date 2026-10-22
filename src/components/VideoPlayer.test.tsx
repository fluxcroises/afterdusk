import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  isTauri: () => true,
  convertFileSrc: (_path: string, protocol: string) => `${protocol}://localhost/`,
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
}));

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { VideoPlayer } = await import("./VideoPlayer");

const clip = {
  id: 7,
  albumId: 1,
  fileName: "C0001.MP4",
  capturedAt: null,
  rating: 0,
  flag: 0,
  color: null,
  width: 1280,
  height: 720,
  orientation: 1,
  hasPreview: true,
  trashedAt: null,
  stackSize: 1,
  companionOf: null,
  kind: "video" as const,
  durationMs: 2000,
};

describe("Lecture d'une vidéo dans la loupe", () => {
  beforeEach(() => invoke.mockReset());

  it("lit l'original d'un codec natif et affiche ses informations", async () => {
    invoke.mockResolvedValue({
      path: "media/7",
      needsProxy: false,
      fps: 25,
      durationMs: 2000,
      codec: "h264",
      ffmpeg: true,
    });
    const { container } = render(<VideoPlayer media={clip} />);
    await act(async () => {});
    const video = container.querySelector("video");
    expect(video?.getAttribute("src")).toBe("video://localhost/media/7");
    expect(video?.getAttribute("poster")).toBe("preview://localhost/media/7/2048?v=0");
    expect(screen.getByText("1280 × 720 · 25 i/s · H.264 · 0:02")).toBeInTheDocument();
    expect(screen.getByText("0:00 / 0:02")).toBeInTheDocument();
    // Image par image : la vidéo se met en pause et avance d'une image.
    const el = video as HTMLVideoElement;
    Object.defineProperty(el, "duration", { value: 2, configurable: true });
    el.pause = vi.fn();
    fireEvent.keyDown(window, { key: "." });
    expect(el.currentTime).toBeCloseTo(0.04);
    expect(el.pause).toHaveBeenCalled();
  });

  it("prépare une version de lecture quand le codec ne se lit pas ici", async () => {
    invoke
      .mockResolvedValueOnce({
        path: null,
        needsProxy: true,
        fps: 50,
        durationMs: 1000,
        codec: "hevc",
        ffmpeg: true,
      })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({
        path: "proxy/7",
        needsProxy: false,
        fps: 50,
        durationMs: 1000,
        codec: "hevc",
        ffmpeg: true,
      });
    const { container } = render(<VideoPlayer media={{ ...clip, durationMs: 1000 }} />);
    await act(async () => {});
    expect(invoke).toHaveBeenCalledWith("video_prepare", { mediaId: 7 });
    expect(container.querySelector("video")?.getAttribute("src")).toBe("video://localhost/proxy/7");
  });

  it("explique quand FFmpeg manque", async () => {
    invoke.mockResolvedValue({
      path: null,
      needsProxy: true,
      fps: 0,
      durationMs: 0,
      codec: "",
      ffmpeg: false,
    });
    render(<VideoPlayer media={clip} />);
    await act(async () => {});
    expect(screen.getByRole("status")).toHaveTextContent("FFmpeg manque");
    expect(invoke).not.toHaveBeenCalledWith("video_prepare", expect.anything());
  });
});
