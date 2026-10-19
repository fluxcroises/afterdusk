import { describe, expect, it } from "vitest";
import { cloudBackupPath, cloudOf } from "./presets";

const onedrive = { service: "onedrive", label: "OneDrive", path: "C:\\Users\\lewis\\OneDrive" };
const icloud = {
  service: "icloud",
  label: "iCloud Drive",
  path: "/Users/lewis/Library/Mobile Documents/com~apple~CloudDocs",
};

describe("sauvegarde dans un dossier cloud", () => {
  it("un sous-dossier Afterdusk, avec le séparateur du système", () => {
    expect(cloudBackupPath(onedrive)).toBe("C:\\Users\\lewis\\OneDrive\\Afterdusk");
    expect(cloudBackupPath({ ...icloud, path: `${icloud.path}/` })).toBe(
      `${icloud.path}/Afterdusk`,
    );
  });

  it("reconnaît une sauvegarde placée dans un dossier cloud, et seulement celle-là", () => {
    const folders = [onedrive, icloud];
    expect(cloudOf(folders, "c:\\users\\lewis\\onedrive\\Afterdusk")?.label).toBe("OneDrive");
    expect(cloudOf(folders, `${icloud.path}/Photos`)?.service).toBe("icloud");
    expect(cloudOf(folders, "C:\\Users\\lewis\\OneDriveCopie")).toBeNull();
    expect(cloudOf(folders, "E:\\Sauvegarde")).toBeNull();
    expect(cloudOf(folders, null)).toBeNull();
  });
});
