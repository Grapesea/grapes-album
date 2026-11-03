import { useEffect, useState } from "react";
import { Image as ImageIcon } from "lucide-react";
import type { AssetView } from "../bindings/types";
import { thumbnailUrl } from "../api/thumbnails";
import { fileExtension } from "../utils/format";

export function AssetThumbnail({
  asset,
  className = "",
}: {
  asset: AssetView;
  className?: string;
}) {
  const [failed, setFailed] = useState(false);
  const source = thumbnailUrl(asset);

  useEffect(() => {
    setFailed(false);
  }, [source]);

  if (!source || failed) {
    return (
      <span className={`asset-thumb fallback ${className}`.trim()}>
        <ImageIcon size={17} />
        <small>{fileExtension(asset.filename)}</small>
      </span>
    );
  }

  return (
    <span className={`asset-thumb ${className}`.trim()}>
      <img
        src={source}
        alt=""
        loading="lazy"
        decoding="async"
        onError={() => setFailed(true)}
      />
    </span>
  );
}
