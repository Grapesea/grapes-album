import { useEffect, useState } from "react";
import { Maximize2, Minimize2, X } from "lucide-react";
import type { AssetView } from "../bindings/types";
import { photoUrl } from "../api/thumbnails";
import { useI18n } from "../i18n";
import { AssetThumbnail } from "./AssetThumbnail";
import { Filmstrip } from "./Filmstrip";

export function PhotoWorkspace({
  asset,
  assets,
  onSelect,
  onClose,
}: {
  asset: AssetView;
  assets: AssetView[];
  onSelect: (asset: AssetView) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [actualSize, setActualSize] = useState(false);
  const [failed, setFailed] = useState(false);
  const source = photoUrl(asset);

  useEffect(() => {
    setFailed(false);
    setActualSize(false);
  }, [asset.id]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
      const index = assets.findIndex((item) => item.id === asset.id);
      if (event.key === "ArrowLeft" && index > 0) onSelect(assets[index - 1]);
      if (event.key === "ArrowRight" && index >= 0 && index < assets.length - 1) {
        onSelect(assets[index + 1]);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [asset.id, assets, onClose, onSelect]);

  return (
    <section className="photo-workspace">
      <header className="photo-workspace-header">
        <div>
          <span className="eyebrow">{t("photoProperties")}</span>
          <strong>{asset.filename}</strong>
        </div>
        <div className="photo-zoom-actions">
          <button
            type="button"
            className={!actualSize ? "active" : ""}
            onClick={() => setActualSize(false)}
            title={t("fit")}
          >
            <Minimize2 size={14} />
            {t("fit")}
          </button>
          <button
            type="button"
            className={actualSize ? "active" : ""}
            onClick={() => setActualSize(true)}
            title={t("actualSize")}
          >
            <Maximize2 size={14} />
            100%
          </button>
          <button type="button" onClick={onClose} title={t("closePhoto")}>
            <X size={16} />
          </button>
        </div>
      </header>
      <div className={`photo-stage ${actualSize ? "actual-size" : ""}`}>
        {source && !failed ? (
          <img
            key={asset.id}
            src={source}
            alt={asset.filename}
            onError={() => setFailed(true)}
          />
        ) : (
          <div className="photo-preview-fallback">
            <AssetThumbnail asset={asset} className="large-preview" />
            <p>{t("previewUnavailable")}</p>
          </div>
        )}
      </div>
      <Filmstrip
        assets={assets}
        selectedAssetId={asset.id}
        onSelect={onSelect}
      />
    </section>
  );
}
