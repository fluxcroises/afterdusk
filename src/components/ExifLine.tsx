import { useTranslation } from "react-i18next";
import { formatAperture, formatFocal, formatIso, formatShutter } from "../ui/format";

interface Exposure {
  iso: number | null;
  focalLength: number | null;
  aperture: number | null;
  shutter: number | null;
}

/** Ligne ISO, focale, ouverture, vitesse sous l'histogramme ; une valeur inconnue est omise. */
export function ExifLine({ exposure }: { exposure: Exposure | null }) {
  const { i18n } = useTranslation();
  const lng = i18n.language;
  const values = exposure
    ? [
        formatIso(exposure.iso),
        formatFocal(exposure.focalLength, lng),
        formatAperture(exposure.aperture, lng),
        formatShutter(exposure.shutter),
      ].filter((v) => v !== null)
    : [];
  return (
    <div className="histogram-exif">
      {values.map((v) => (
        <span key={v}>{v}</span>
      ))}
    </div>
  );
}
