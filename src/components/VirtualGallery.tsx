import { useEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { CalendarDays, Camera } from "lucide-react";
import type { AssetView } from "../bindings/types";
import { AssetTags } from "./AssetTags";
import { AssetThumbnail } from "./AssetThumbnail";

const GAP = 12;
const MEDIUM_CARD_WIDTH = 168;
const SMALL_CARD_WIDTH = 126;

export function VirtualGallery({
  assets,
  selectedAssetId,
  onSelect,
  onOpen,
  compact = false,
}: {
  assets: AssetView[];
  selectedAssetId: string | null;
  onSelect: (asset: AssetView) => void;
  onOpen: (asset: AssetView) => void;
  compact?: boolean;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(720);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      setWidth(entry.contentRect.width);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const columns = Math.max(
    1,
    Math.floor(
      (width + GAP) /
        ((compact ? SMALL_CARD_WIDTH : MEDIUM_CARD_WIDTH) + GAP),
    ),
  );
  const cardWidth = (width - GAP * (columns - 1)) / columns;
  const cardHeight = cardWidth * 0.7 + (compact ? 54 : 76);
  const rowCount = Math.ceil(assets.length / columns);

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => containerRef.current,
    estimateSize: () => cardHeight + GAP,
    overscan: 3,
  });

  return (
    <div className="gallery-scroll" ref={containerRef}>
      <div
        className="gallery-virtual-space"
        style={{ height: `${virtualizer.getTotalSize()}px` }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const start = virtualRow.index * columns;
          const rowAssets = assets.slice(start, start + columns);
          return (
            <div
              className="gallery-row"
              key={virtualRow.key}
              style={{
                height: `${cardHeight}px`,
                transform: `translateY(${virtualRow.start}px)`,
                gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
              }}
            >
              {rowAssets.map((asset) => (
                <button
                  type="button"
                  className={`gallery-card ${compact ? "compact" : ""} ${
                    selectedAssetId === asset.id ? "selected" : ""
                  }`}
                  key={asset.id}
                  onClick={() => onSelect(asset)}
                  onDoubleClick={() => onOpen(asset)}
                  title={asset.filename}
                >
                  <AssetThumbnail asset={asset} className="gallery-thumbnail" />
                  <span className="gallery-card-info">
                    <strong>{asset.filename}</strong>
                    <span className="gallery-card-meta">
                      {asset.metadata.cameraModel && (
                        <span>
                          <Camera size={10} />
                          {asset.metadata.cameraModel}
                        </span>
                      )}
                      {asset.metadata.capturedAt && (
                        <span>
                          <CalendarDays size={10} />
                          {asset.metadata.capturedAt.slice(0, 10)}
                        </span>
                      )}
                    </span>
                    <AssetTags tags={asset.tags} limit={1} />
                  </span>
                </button>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
