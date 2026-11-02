import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, RefreshCw, RotateCcw } from "lucide-react";
import {
  getGraphCatalog,
  getGraphView,
  saveGraphPosition,
} from "../api/workspace";
import type {
  AssetView,
  GraphInfo,
  GraphView,
} from "../bindings/types";
import { useI18n } from "../i18n";
import { SigmaGraph } from "../graph/SigmaGraph";

const ROOT_GRAPH_ID = "graphspaces";
type Translator = ReturnType<typeof useI18n>["t"];

export function GraphWorkspace({
  assets,
  refreshToken,
  onClose,
  onOpenAsset,
  onError,
}: {
  assets: AssetView[];
  refreshToken: string;
  onClose: () => void;
  onOpenAsset: (asset: AssetView) => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const [catalog, setCatalog] = useState<GraphInfo[]>([]);
  const [view, setView] = useState<GraphView | null>(null);
  const [graphId, setGraphId] = useState(ROOT_GRAPH_ID);
  const [loading, setLoading] = useState(true);
  const [layoutNonce, setLayoutNonce] = useState(0);

  const load = useCallback(
    async (targetId: string) => {
      setLoading(true);
      try {
        const [nextCatalog, nextView] = await Promise.all([
          getGraphCatalog(),
          getGraphView(targetId),
        ]);
        setCatalog(nextCatalog);
        setView(nextView);
        setGraphId(nextView.graph.id);
        setLayoutNonce(0);
      } catch (caught) {
        onError(normalizeError(caught));
      } finally {
        setLoading(false);
      }
    },
    [onError],
  );

  useEffect(() => {
    void load(graphId);
  }, [graphId, load, refreshToken]);

  const openAssetById = (assetId: string) => {
    const asset = assets.find((item) => item.id === assetId);
    if (asset) onOpenAsset(asset);
  };

  const goBack = () => {
    const parent = view?.breadcrumbs.at(-2);
    if (parent) setGraphId(parent.id);
    else onClose();
  };

  const savePositions = useCallback(
    async (
      positions: Array<{
        nodeKey: string;
        x: number;
        y: number;
        pinned: boolean;
      }>,
    ) => {
      if (!view) return;
      try {
        await Promise.all(
          positions.map((position) =>
            saveGraphPosition(
              view.graph.id,
              position.nodeKey,
              position.x,
              position.y,
              position.pinned,
            ),
          ),
        );
      } catch (caught) {
        onError(normalizeError(caught));
      }
    },
    [onError, view],
  );

  return (
    <section className="graph-workspace">
      <header className="graph-toolbar">
        <button
          type="button"
          className="icon-button"
          title={t("graphRelayout")}
          onClick={() => setLayoutNonce((nonce) => nonce + 1)}
          disabled={loading}
        >
          <RotateCcw size={15} />
        </button>
        <button
          type="button"
          className="graph-back-button"
          onClick={goBack}
          title={t("backToFiles")}
        >
          <ArrowLeft size={16} />
        </button>
        <nav className="graph-breadcrumbs" aria-label={t("graphBreadcrumb")}>
          {view?.breadcrumbs.map((graph, index) => (
            <span key={graph.id}>
              {index > 0 && <span className="graph-breadcrumb-separator">/</span>}
              <button
                type="button"
                className={graph.id === graphId ? "current" : ""}
                onClick={() => setGraphId(graph.id)}
              >
                {graphLabel(graph, catalog, t)}
              </button>
            </span>
          ))}
        </nav>
        <label className="graph-selector">
          <span>{t("graph")}</span>
          <select
            value={graphId}
            onChange={(event) => setGraphId(event.target.value)}
          >
            {catalog.map((graph) => (
              <option key={graph.id} value={graph.id}>
                {graphLabel(graph, catalog, t)}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="icon-button"
          title={t("graphRefresh")}
          onClick={() => void load(graphId)}
          disabled={loading}
        >
          <RefreshCw size={15} className={loading ? "spin" : ""} />
        </button>
      </header>

      <div className="graph-stage">
        {view && view.nodes.length > 0 ? (
          <SigmaGraph
            view={view}
            layoutNonce={layoutNonce}
            onNavigate={setGraphId}
            onOpenAsset={openAssetById}
            onSavePositions={savePositions}
          />
        ) : (
          <div className="graph-empty">
            <strong>{t("graphEmpty")}</strong>
            <p>{t("graphEmptyHint")}</p>
          </div>
        )}
      </div>

      <footer className="graph-legend">
        <span><i className="portal-dot" />{t("portalNode")}</span>
        <span><i className="asset-dot" />{t("photoNode")}</span>
        <span>{t("graphHint")}</span>
      </footer>
    </section>
  );
}

function graphLabel(
  graph: GraphInfo,
  catalog: GraphInfo[],
  t: Translator,
) {
  if (graph.kind === "root") return "GraphSpaces";
  const dimension = graph.dimension ? dimensionLabel(graph.dimension, t) : graph.name;
  if (graph.kind === "value") {
    const parent = catalog.find((item) => item.id === graph.parentId);
    return `${parent?.dimension ? dimensionLabel(parent.dimension, t) : dimension} · ${graph.name}`;
  }
  return dimension;
}

export function dimensionLabel(
  dimension: string,
  t: Translator,
) {
  const keys = {
    date: "dimensionDate",
    time: "dimensionTime",
    weather: "dimensionWeather",
    location: "dimensionLocation",
    type: "dimensionType",
    camera: "dimensionCamera",
    lens: "dimensionLens",
    tone: "dimensionTone",
    custom: "dimensionCustom",
  } as const;
  const key = keys[dimension as keyof typeof keys];
  return key ? t(key) : dimension;
}

function normalizeError(error: unknown) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "GraphSpaces failed to load.";
}
