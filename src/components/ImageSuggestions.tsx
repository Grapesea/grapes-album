import { type ReactNode, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AssetView, Dimension } from "../bindings/types";
import { classificationValueDisplayName } from "../classificationLabels";
import { useI18n } from "../i18n";

export function ImageSuggestions({ asset, dimensions, kind, busy, onTags, onValue, render }: {
  asset: AssetView; dimensions: Dimension[]; kind: "tags" | "values"; busy: boolean;
  render?: (button: ReactNode, results: ReactNode) => ReactNode;
  onTags: (tags: string[]) => Promise<boolean>;
  onValue: (id: string) => Promise<boolean>;
}) {
  const { locale } = useI18n();
  const [result, setResult] = useState<{ tags: string[]; valueIds: string[] }>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const activeRequest = useRef<string | null>(null);
  useEffect(() => () => {
    const requestId = activeRequest.current;
    activeRequest.current = null;
    if (requestId) void invoke("cancel_recommend_asset", { requestId }).catch(() => {});
  }, []);
  const button = <button type="button" className={`image-suggestion-button ${render ? "classification-title-action" : ""}`}
    disabled={!loading && (busy || !asset.hasThumbnail)}
    title={loading ? "再次点击停止分析" : "将图片预览发送至已配置的 Ask Grapes 多模态模型"}
    onClick={async () => {
      if (activeRequest.current) {
        const requestId = activeRequest.current;
        activeRequest.current = null;
        setLoading(false);
        try { await invoke("cancel_recommend_asset", { requestId }); }
        catch (e) { if (!activeRequest.current) setError(String(e)); }
        return;
      }
      const requestId = crypto.randomUUID();
      activeRequest.current = requestId;
      setLoading(true); setError(""); setResult(undefined);
      try {
        const suggestions = await invoke<{ tags: string[]; valueIds: string[] }>(
          "recommend_asset", { assetId: asset.id, requestId });
        if (activeRequest.current === requestId) setResult(suggestions);
      } catch (e) {
        if (activeRequest.current === requestId) setError(String(e));
      } finally {
        if (activeRequest.current === requestId) {
          activeRequest.current = null;
          setLoading(false);
        }
      }
    }}>
      {loading ? "停止分析" : "智能推荐"}
      {loading && <span className="image-suggestion-progress" role="progressbar"
        aria-label="正在分析图片"><span /></span>}
    </button>;
  const results = <div aria-live="polite">
    {!asset.hasThumbnail && <small>缺少图片预览，暂不推荐</small>}
    {error && <small role="alert">{error}</small>}
    {result && (kind === "tags" ? result.tags.filter(tag => !asset.tags.includes(tag)).map(tag =>
      <button key={tag} disabled={busy} onClick={() => void onTags([...asset.tags, tag])}>+ {tag}</button>
    ) : result.valueIds.filter(id => !asset.classifications.some(v => v.valueId === id)).map(id => {
      const value = dimensions.flatMap(d => d.values).find(v => v.id === id);
      return value && <button key={id} disabled={busy} onClick={() => void onValue(id)}>
        + {classificationValueDisplayName(id, value.name, locale)}
      </button>;
    }))}
    {result && (kind === "tags" ? result.tags.length === 0 : result.valueIds.length === 0) && <small>暂无可靠建议</small>}
  </div>;
  return render ? render(button, results) : <>{button}{results}</>;
}
