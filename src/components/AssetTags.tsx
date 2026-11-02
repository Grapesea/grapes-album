import { useLayoutEffect, useRef, useState } from "react";
import { Tag } from "lucide-react";
import { useI18n } from "../i18n";

export function AssetTags({
  tags,
  limit = 2,
  adaptive = false,
  moreLabel,
}: {
  tags: string[];
  limit?: number;
  adaptive?: boolean;
  moreLabel?: string;
}) {
  const { t } = useI18n();
  const containerRef = useRef<HTMLSpanElement>(null);
  const measureRef = useRef<HTMLSpanElement>(null);
  const [visibleCount, setVisibleCount] = useState(
    Math.min(limit, tags.length),
  );

  useLayoutEffect(() => {
    if (!adaptive) {
      setVisibleCount(Math.min(limit, tags.length));
      return;
    }

    const container = containerRef.current;
    const measure = measureRef.current;
    if (!container || !measure) return;

    const recalculate = () => {
      const available = container.clientWidth;
      if (available <= 0) return;

      const pills = Array.from(measure.children).filter(
        (_, index) => index < tags.length,
      ) as HTMLElement[];
      const widths = pills.map((pill) => pill.getBoundingClientRect().width);
      const moreElement = measure.lastElementChild as HTMLElement | null;
      const moreWidth = moreElement?.getBoundingClientRect().width ?? 0;
      const gap = 4;

      let count = 0;
      let used = 0;
      for (const width of widths) {
        const next = width + (count > 0 ? gap : 0);
        if (used + next > available) break;
        used += next;
        count += 1;
      }

      if (count < tags.length) {
        while (
          count > 0 &&
          used + gap + moreWidth > available
        ) {
          count -= 1;
          used -= widths[count] + (count > 0 ? gap : 0);
        }
      }
      setVisibleCount(count);
    };

    recalculate();
    const observer = new ResizeObserver(recalculate);
    observer.observe(container);
    return () => observer.disconnect();
  }, [adaptive, limit, tags]);

  if (tags.length === 0) {
    return <span className="tag-empty">{t("noTags")}</span>;
  }

  const visible = tags.slice(0, visibleCount);
  const hiddenCount = tags.length - visible.length;
  return (
    <span
      className={`asset-tags ${adaptive ? "adaptive" : ""}`}
      ref={containerRef}
      title={tags.join(" · ")}
    >
      {adaptive && (
        <span className="asset-tags-measure" ref={measureRef} aria-hidden="true">
          {tags.map((tag) => (
            <span className="tag-pill" key={tag}>
              <Tag size={10} />
              {tag}
            </span>
          ))}
          <span className="tag-more">+20</span>
        </span>
      )}
      {visible.map((tag) => (
        <span className="tag-pill" key={tag}>
          <Tag size={10} />
          {tag}
        </span>
      ))}
      {hiddenCount > 0 && (
        <span className="tag-more">{moreLabel ?? `+${hiddenCount}`}</span>
      )}
    </span>
  );
}
