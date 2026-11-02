import { Tag } from "lucide-react";
import { useI18n } from "../i18n";

export function AssetTags({
  tags,
  limit = 2,
}: {
  tags: string[];
  limit?: number;
}) {
  const { t } = useI18n();
  if (tags.length === 0) {
    return <span className="tag-empty">{t("noTags")}</span>;
  }

  const visible = tags.slice(0, limit);
  const hiddenCount = tags.length - visible.length;
  return (
    <span className="asset-tags" title={tags.join(" · ")}>
      {visible.map((tag) => (
        <span className="tag-pill" key={tag}>
          <Tag size={10} />
          {tag}
        </span>
      ))}
      {hiddenCount > 0 && (
        <span className="tag-more">+{hiddenCount}</span>
      )}
    </span>
  );
}
