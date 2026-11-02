import { ChevronRight, Plus, X } from "lucide-react";
import type { CSSProperties } from "react";
import type {
  AssetClassification,
  Dimension,
} from "../bindings/types";
import { useI18n } from "../i18n";
import {
  classificationValueDisplayName,
  dimensionDisplayName,
} from "../classificationLabels";

export function ClassificationEditor({
  classifications,
  dimensions,
  busy,
  onToggle,
  onCreateValue,
}: {
  classifications: AssetClassification[];
  dimensions: Dimension[];
  busy: boolean;
  onToggle: (valueId: string, selected: boolean) => Promise<boolean>;
  onCreateValue: (dimensionId: string, name: string) => Promise<boolean>;
}) {
  const { locale, t } = useI18n();

  return (
    <div className="classification-editor">
      {dimensions.map((dimension) => {
        const selectedValues = classifications.filter(
          (classification) => classification.dimensionId === dimension.id,
        );
        const availableValues = dimension.values.filter(
          (value) =>
            !selectedValues.some(
              (classification) => classification.valueId === value.id,
            ),
        );
        return (
          <details className="classification-dimension" key={dimension.id}>
            <summary>
              <ChevronRight size={12} />
              <span>
                {dimensionDisplayName(dimension.id, dimension.name, locale)}
              </span>
              <small>
                {selectedValues.length > 0
                  ? selectedValues
                      .map((classification) =>
                        classificationValueDisplayName(
                          classification.valueId,
                          classification.valueName,
                          locale,
                        ),
                      )
                      .join(" · ")
                  : t("unclassified")}
              </small>
            </summary>
            <div className="classification-values">
              {selectedValues.map((classification) => (
                <span
                  className="classification-chip selected"
                  key={classification.valueId}
                  style={
                    classification.color
                      ? ({ "--tag-color": classification.color } as CSSProperties)
                      : undefined
                  }
                >
                  {classificationValueDisplayName(
                    classification.valueId,
                    classification.valueName,
                    locale,
                  )}
                  <button
                    type="button"
                    title={t("removeClassification")}
                    disabled={busy}
                    onClick={() =>
                      void onToggle(classification.valueId, false)
                    }
                  >
                    <X size={10} />
                  </button>
                </span>
              ))}
              <span className="classification-picker-row">
                <select
                  value=""
                  disabled={busy || availableValues.length === 0}
                  aria-label={t("addClassificationValue")}
                  onChange={(event) => {
                    if (event.target.value) {
                      void onToggle(event.target.value, true);
                    }
                  }}
                >
                  <option value="">
                    {availableValues.length > 0
                      ? t("addClassificationValue")
                      : t("noMoreValues")}
                  </option>
                  {availableValues.map((value) => (
                    <option value={value.id} key={value.id}>
                      {classificationValueDisplayName(
                        value.id,
                        value.name,
                        locale,
                      )}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  className="classification-add-value"
                  disabled={busy}
                  onClick={async () => {
                    const name = window.prompt(t("newValuePrompt"));
                    if (name) await onCreateValue(dimension.id, name);
                  }}
                >
                  <Plus size={10} />
                  {t("addValue")}
                </button>
              </span>
            </div>
          </details>
        );
      })}
    </div>
  );
}
