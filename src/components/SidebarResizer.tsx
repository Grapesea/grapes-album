import { useEffect, useRef } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

export function SidebarResizer({
  side,
  width,
  min,
  max,
  defaultWidth,
  onChange,
}: {
  side: "left" | "right";
  width: number;
  min: number;
  max: number;
  defaultWidth: number;
  onChange: (width: number) => void;
}) {
  const widthRef = useRef(width);
  const maxRef = useRef(max);

  useEffect(() => {
    widthRef.current = width;
  }, [width]);

  useEffect(() => {
    maxRef.current = max;
  }, [max]);

  const clamp = (value: number) =>
    Math.round(Math.min(maxRef.current, Math.max(min, value)));

  const startResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = widthRef.current;
    document.body.classList.add("sidebar-resizing");

    const onMove = (moveEvent: PointerEvent) => {
      const delta =
        side === "left"
          ? moveEvent.clientX - startX
          : startX - moveEvent.clientX;
      const next = clamp(startWidth + delta);
      widthRef.current = next;
      onChange(next);
    };

    const onUp = () => {
      document.body.classList.remove("sidebar-resizing");
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  };

  const reset = () => {
    const next = clamp(defaultWidth);
    widthRef.current = next;
    onChange(next);
  };

  return (
    <div
      className={`sidebar-resizer sidebar-resizer-${side}`}
      role="separator"
      aria-orientation="vertical"
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={Math.round(width)}
      tabIndex={0}
      title="拖拽调整侧边栏宽度，双击恢复默认"
      onPointerDown={startResize}
      onDoubleClick={reset}
      onKeyDown={(event) => {
        if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
        event.preventDefault();
        const direction =
          side === "left"
            ? event.key === "ArrowLeft"
              ? -12
              : 12
            : event.key === "ArrowRight"
              ? -12
              : 12;
        const next = clamp(widthRef.current + direction);
        widthRef.current = next;
        onChange(next);
      }}
    >
      <span />
    </div>
  );
}
