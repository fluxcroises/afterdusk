import { useTranslation } from "react-i18next";
import type { SectionOption } from "../ui/sections";

interface SectionPickerProps {
  options: SectionOption[];
  checked: string[];
  onChange: (keys: string[]) => void;
}

/** Cases à cocher des familles de réglages, avec « Tout » et « Rien ». */
export function SectionPicker({ options, checked, onChange }: SectionPickerProps) {
  const { t } = useTranslation();
  const toggle = (key: string, on: boolean) =>
    onChange(on ? [...checked, key] : checked.filter((k) => k !== key));
  return (
    <fieldset className="section-picker">
      <legend>{t("develop.sections.legend")}</legend>
      <div className="section-picker-all">
        <button
          type="button"
          className="button button-small"
          onClick={() => onChange(options.map((o) => o.key))}
        >
          {t("develop.sections.all")}
        </button>
        <button type="button" className="button button-small" onClick={() => onChange([])}>
          {t("develop.sections.none")}
        </button>
      </div>
      {options.map((o) => (
        <label key={o.key} className="check-row">
          <input
            type="checkbox"
            checked={checked.includes(o.key)}
            onChange={(e) => toggle(o.key, e.currentTarget.checked)}
          />
          {t(`develop.sections.${o.key}`)}
        </label>
      ))}
    </fieldset>
  );
}
