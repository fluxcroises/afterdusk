// @vitest-environment node
import { describe, expect, it } from "vitest";
// @ts-expect-error -- script Node en JavaScript, sans déclaration de types
import { latestJson } from "./latest-json.mjs";

describe("latest.json des mises à jour", () => {
  it("une entrée par plateforme, macOS universel pour les deux processeurs", () => {
    const json = latestJson("0.5.0", (name: string) => `sig:${name}\n`, "2026-10-23T12:00:00Z");
    const base = "https://github.com/lewisbedar/afterdusk-app/releases/download/v0.5.0";
    expect(json).toEqual({
      version: "0.5.0",
      notes: "Afterdusk 0.5.0",
      pub_date: "2026-10-23T12:00:00Z",
      platforms: {
        "windows-x86_64": {
          signature: "sig:Afterdusk_0.5.0_x64-setup.exe",
          url: `${base}/Afterdusk_0.5.0_x64-setup.exe`,
        },
        "darwin-aarch64": {
          signature: "sig:Afterdusk_0.5.0_universal.app.tar.gz",
          url: `${base}/Afterdusk_0.5.0_universal.app.tar.gz`,
        },
        "darwin-x86_64": {
          signature: "sig:Afterdusk_0.5.0_universal.app.tar.gz",
          url: `${base}/Afterdusk_0.5.0_universal.app.tar.gz`,
        },
      },
    });
  });
});
