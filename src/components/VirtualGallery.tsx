import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";
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
  scrollElementRef,
}: {
  assets: AssetView[];
  selectedAssetId: string | null;
  onSelect: (asset: AssetView) => void;
  onOpen: (asset: AssetView) => void;
  compact?: boolean;
  scrollElementRef: RefObject<HTMLElement | null>;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(720);
  const [scrollMargin, setScrollMargin] = useState(0);

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
    getScrollElement: () => scrollElementRef.current,
    estimateSize: () => cardHeight + GAP,
    overscan: 3,
    scrollMargin,
  });

  useLayoutEffect(() => {
    const gallery = containerRef.current;
    const scrollElement = scrollElementRef.current;
    if (!gallery || !scrollElement) return;

    const updateMargin = () => {
      const galleryRect = gallery.getBoundingClientRect();
      const scrollRect = scrollElement.getBoundingClientRect();
      setScrollMargin(
        galleryRect.top - scrollRect.top + scrollElement.scrollTop,
      );
    };

    updateMargin();
    const observer = new ResizeObserver(updateMargin);
    observer.observe(gallery);
    observer.observe(scrollElement);
    return () => observer.disconnect();
  }, [scrollElementRef, width, assets.length]);

  const virtualSize = Math.max(
    0,
    virtualizer.getTotalSize() - scrollMargin,
  );

  return (
    <div className="gallery-scroll" ref={containerRef}>
      <div
        className="gallery-virtual-space"
        style={{ height: `${virtualSize}px` }}
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
                transform: `translateY(${virtualRow.start - scrollMargin}px)`,
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
                    <AssetTags
                      tags={asset.tags}
                      limit={compact ? 1 : undefined}
                      adaptive={!compact}
                      moreLabel={compact ? undefined : "+1"}
                      availableWidth={
                        compact ? undefined : Math.max(0, cardWidth - 20)
                      }
                    />
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
