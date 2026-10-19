import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

const { default: i18n } = await import("../i18n");
await i18n.changeLanguage("fr");
const { useImport } = await import("../store/import");
const { OfflineBanner } = await import("./OfflineBanner");

const library = (isOnline: boolean) => ({
  id: 1,
  name: "Photos",
  rootPath: "E:\\Photos",
  backupPath: null,
  folderTemplate: "{yyyy}/{album}",
  renameTemplate: null,
  isOnline,
});

describe("bibliothèque débranchée", () => {
  beforeEach(() => {
    useImport.setState({ libraries: [library(true)], libraryId: 1 });
  });

  it("rien tant que le disque est là", () => {
    render(<OfflineBanner />);
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("le bandeau dit ce qui attend le disque, puis annonce son retour", () => {
    useImport.setState({ libraries: [library(false)] });
    render(<OfflineBanner />);
    const banner = screen.getByRole("status");
    expect(banner).toHaveTextContent("« Photos » est hors ligne.");
    expect(banner).toHaveTextContent("Rebranchez le disque (E:\\Photos)");
    expect(banner).toHaveTextContent("Vous pouvez parcourir et noter vos photos.");

    act(() => useImport.setState({ libraries: [library(true)] }));
    expect(screen.getByRole("status")).toHaveTextContent("« Photos » est de nouveau disponible.");
  });
});
