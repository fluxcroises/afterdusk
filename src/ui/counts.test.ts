import { describe, expect, it } from "vitest";

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { mediaCount } = await import("./counts");

describe("compte des médias", () => {
  it("photos et vidéos comptées à part", () => {
    const t = i18n.t.bind(i18n);
    expect(mediaCount(t, 11, 2)).toBe("11 photos et 2 vidéos");
    expect(mediaCount(t, 1, 0)).toBe("1 photo");
    expect(mediaCount(t, 0, 1)).toBe("1 vidéo");
    expect(t("status.selection", { count: 1, items: mediaCount(t, 11, 2) })).toBe(
      "11 photos et 2 vidéos · 1 sélectionnée",
    );
  });
});
