import icon from "../../assets/icon.svg";

export function BrandMark({ className = "" }: { className?: string }) {
  return (
    <img
      className={`brand-mark ${className}`.trim()}
      src={icon}
      alt=""
      aria-hidden="true"
      draggable={false}
    />
  );
}
