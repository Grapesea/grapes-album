import { useEffect, useRef } from "react";
import type { AssetView } from "../bindings/types";
import { AssetThumbnail } from "./AssetThumbnail";

export function Filmstrip({
  assets,
  selectedAssetId,
  onSelect,
}: {
  assets: AssetView[];
  selectedAssetId: string;
  onSelect: (asset: AssetView) => void;
}) {
  const stripRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const selected = stripRef.current?.querySelector<HTMLElement>(
      `[data-asset-id="${selectedAssetId}"]`,
    );
    selected?.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "center" });
  }, [selectedAssetId]);

  return (
    <div className="filmstrip" ref={stripRef}>
      {assets.map((asset) => (
        <button
          type="button"
          key={asset.id}
          data-asset-id={asset.id}
          className={`filmstrip-item ${
            asset.id === selectedAssetId ? "selected" : ""
          }`}
          onClick={() => onSelect(asset)}
          title={asset.filename}
        >
          <AssetThumbnail asset={asset} />
          <span>{asset.filename}</span>
        </button>
      ))}
    </div>
  );
}
