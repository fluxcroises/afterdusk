import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

const { default: i18n } = await import("../i18n");
const { useLibrary } = await import("../store/library");
const { PhotoInfoPanel } = await import("./CatalogPanels");

const details = {
  id: 1,
  fileName: "DSC_0001.NEF",
  originalName: "DSC_0001.NEF",
  path: "2026/DSC_0001.NEF",
  capturedAt: "2026-07-14T12:00:00",
  utcOffset: "+02:00",
  width: 6048,
  height: 4024,
  camera: "Nikon Z 6",
  lens: null,
  focalLength: null,
  focalLength35mm: null,
  aperture: null,
  shutter: null,
  iso: null,
  exposureCompensation: null,
  keywords: [],
  stackFiles: [],
  latitude: 49.2869,
  longitude: -0.1166,
  locationSource: "gpx" as const,
  city: "Cabourg",
  region: "Normandy",
  country: "FR",
  capturedAtCamera: null,
};

describe("fiche Informations : lieu", () => {
  afterEach(() => useLibrary.setState({ details: null }));

  it("coordonnées, hémisphères et source, en français", async () => {
    await i18n.changeLanguage("fr");
    useLibrary.setState({ details });
    render(<PhotoInfoPanel scope="library" />);
    expect(screen.getByText("Lieu").nextSibling).toHaveTextContent("Cabourg, France");
    expect(screen.getByText("49,2869° N, 0,1166° O")).toBeInTheDocument();
    expect(screen.getByText("Origine du lieu").nextSibling).toHaveTextContent("Trace GPX");
  });

  it("en anglais, et sans lieu", async () => {
    await i18n.changeLanguage("en");
    useLibrary.setState({ details: { ...details, locationSource: "exif" } });
    const { rerender } = render(<PhotoInfoPanel scope="library" />);
    expect(screen.getByText("Location").nextSibling).toHaveTextContent("Cabourg, France");
    expect(screen.getByText("49.2869° N, 0.1166° W")).toBeInTheDocument();
    expect(screen.getByText("Location source").nextSibling).toHaveTextContent("Camera GPS");
    useLibrary.setState({
      details: {
        ...details,
        latitude: null,
        longitude: null,
        locationSource: null,
        city: null,
        region: null,
        country: null,
      },
    });
    rerender(<PhotoInfoPanel scope="library" />);
    expect(screen.getByText("Location").nextSibling).toHaveTextContent("Not set");
    expect(screen.queryByText("Location source")).not.toBeInTheDocument();
    await i18n.changeLanguage("fr");
  });
});
