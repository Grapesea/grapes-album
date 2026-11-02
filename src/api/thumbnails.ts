import { convertFileSrc } from "@tauri-apps/api/core";
import type { AssetView } from "../bindings/types";
import { desktopRuntime } from "./workspace";

export function thumbnailUrl(asset: AssetView): string | null {
  if (!desktopRuntime || !asset.hasThumbnail) return null;
  return `${convertFileSrc(asset.id, "grapes-thumb")}?v=${asset.mtimeNs}`;
}
