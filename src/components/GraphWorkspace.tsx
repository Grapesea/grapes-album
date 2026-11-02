import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowLeft, LoaderCircle, RefreshCw, RotateCcw } from "lucide-react";
import {
  createSmartGraph,
  deleteSmartGraph,
  getClassificationSchema,
  getGraphCatalog,
  getGraphView,
  saveGraphPosition,
} from "../api/workspace";
import type {
  AssetView,
  Dimension,
  GraphInfo,
  GraphView,
  SmartGraphFilter,
} from "../bindings/types";
import { classificationValueDisplayName } from "../classificationLabels";
import { useI18n } from "../i18n";
import { SigmaGraph } from "../graph/SigmaGraph";

type Translator = ReturnType<typeof useI18n>["t"];

export function GraphWorkspace({
  workspaceId,
  assets,
  refreshToken,
  graphId,
  onNavigate,
  onClose,
  onOpenAsset,
  onError,
}: {
  workspaceId: string;
  assets: AssetView[];
  refreshToken: string;
  graphId: string;
  onNavigate: (graphId: string) => void;
  onClose: () => void;
  onOpenAsset: (asset: AssetView, memberIds: string[]) => void;
  onError: (message: string) => void;
}) {
  const { locale, t } = useI18n();
  const [catalog, setCatalog] = useState<GraphInfo[]>([]);
  const [classificationDimensions, setClassificationDimensions] = useState<
    Dimension[]
  >([]);
  const [view, setView] = useState<GraphView | null>(null);
  const [loading, setLoading] = useState(true);
  const [layoutNonce, setLayoutNonce] = useState(0);
  const requestId = useRef(0);

  const load = useCallback(
    async (targetId: string) => {
      const request = ++requestId.current;
      setLoading(true);
      try {
        const [nextCatalog, nextView, nextDimensions] = await Promise.all([
          getGraphCatalog(),
          getGraphView(targetId),
          getClassificationSchema(),
        ]);
        if (request !== requestId.current) return;
        setCatalog(nextCatalog);
        setView(nextView);
        setClassificationDimensions(nextDimensions);
        setLayoutNonce(0);
      } catch (caught) {
        if (request === requestId.current) onError(normalizeError(caught));
      } finally {
        if (request === requestId.current) setLoading(false);
      }
    },
    [onError],
  );

  useEffect(() => {
    void load(graphId);
    return () => {
      requestId.current += 1;
    };
  }, [graphId, load, refreshToken, workspaceId]);

  const openAssetById = (assetId: string) => {
    const asset = assets.find((item) => item.id === assetId);
    if (asset)
      onOpenAsset(
        asset,
        view?.nodes.flatMap((node) => (node.assetId ? [node.assetId] : [])) ??
          [],
      );
  };

  const goBack = () => {
    const parent = view?.breadcrumbs.at(-2);
    if (parent) onNavigate(parent.id);
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
        throw caught;
      }
    },
    [onError, view],
  );

  const createCombinedSmartGraph = useCallback(
    async (name: string, filters: SmartGraphFilter[]) => {
      try {
        const graph = await createSmartGraph(name, filters);
        onNavigate(graph.id);
        return true;
      } catch (caught) {
        onError(normalizeError(caught));
        return false;
      }
    },
    [onError, onNavigate],
  );

  const deleteCombinedSmartGraph = useCallback(
    async (id: string) => {
      try {
        const nextCatalog = await deleteSmartGraph(id);
        setCatalog(nextCatalog);
        if (graphId === id) onNavigate("graphspaces");
        else await load(graphId);
        return true;
      } catch (caught) {
        onError(normalizeError(caught));
        return false;
      }
    },
    [graphId, load, onError, onNavigate],
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
          title={t(
            view && view.breadcrumbs.length > 1 ? "graphBack" : "backToFiles",
          )}
          aria-label={t(
            view && view.breadcrumbs.length > 1 ? "graphBack" : "backToFiles",
          )}
        >
          <ArrowLeft size={16} />
        </button>
        <nav className="graph-breadcrumbs" aria-label={t("graphBreadcrumb")}>
          {view?.breadcrumbs.map((graph, index) => (
            <span key={graph.id}>
              {index > 0 && (
                <span className="graph-breadcrumb-separator">/</span>
              )}
              <button
                type="button"
                className={graph.id === graphId ? "current" : ""}
                onClick={() => onNavigate(graph.id)}
                aria-current={graph.id === graphId ? "page" : undefined}
              >
                {graphLabel(graph, catalog, t, locale)}
              </button>
            </span>
          ))}
        </nav>
        <label className="graph-selector">
          <span>{t("graph")}</span>
          <select
            value={graphId}
            onChange={(event) => onNavigate(event.target.value)}
          >
            {catalog.map((graph) => (
              <option key={graph.id} value={graph.id}>
                {graphLabel(graph, catalog, t, locale)}
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

      <div className="graph-stage" aria-busy={loading}>
        {loading ? (
          <div className="graph-loading" role="status">
            <LoaderCircle size={18} className="spin" />
            {t("graphLoading")}
          </div>
        ) : view && view.graph.id === graphId ? (
          <>
            <div className="graph-stage-heading">
              <strong>{graphLabel(view.graph, catalog, t, locale)}</strong>
              <span>
                {t("graphNodeCount", {
                  count: view.nodes.filter((node) => !node.dummy).length,
                })}{" "}
                · {t("photoCount", { count: view.graph.photoCount })}
              </span>
            </div>
            <SigmaGraph
              key={`${workspaceId}:${view.graph.id}`}
              workspaceId={workspaceId}
              view={view}
              layoutNonce={layoutNonce}
              classificationDimensions={classificationDimensions}
              smartGraphs={catalog.filter((graph) => graph.kind === "smart")}
              onNavigate={onNavigate}
              onOpenAsset={openAssetById}
              onSavePositions={savePositions}
              onCreateSmartGraph={createCombinedSmartGraph}
              onDeleteSmartGraph={deleteCombinedSmartGraph}
            />
            {!view.nodes.some((node) => !node.dummy) && (
              <div className="graph-empty graph-empty-overlay">
                <strong>{t("graphEmpty")}</strong>
                <p>{t("graphEmptyHint")}</p>
              </div>
            )}
          </>
        ) : (
          <div className="graph-empty">
            <strong>{t("graphEmpty")}</strong>
            <p>{t("graphEmptyHint")}</p>
          </div>
        )}
      </div>

      <footer className="graph-legend">
        <span>
          <i className="dummy-dot" />
          {t("graphRoot")}
        </span>
        <span>
          <i className="family-dot capture" />
          {t("graphCaptureGroup")}
        </span>
        <span>
          <i className="family-dot classification" />
          {t("graphClassificationGroup")}
        </span>
        <span>
          <i className="family-dot tag" />
          {t("graphTagGroup")}
        </span>
        <span>
          <i className="asset-dot" />
          {t("photoNode")}
        </span>
      </footer>
    </section>
  );
}

function graphLabel(
  graph: GraphInfo,
  catalog: GraphInfo[],
  t: Translator,
  locale: ReturnType<typeof useI18n>["locale"],
) {
  if (graph.kind === "root") return "GraphSpaces";
  const dimension = graph.dimension
    ? dimensionDisplay(graph.dimension, graph.name, t)
    : graph.name;
  if (graph.kind === "value") {
    const parent = catalog.find((item) => item.id === graph.parentId);
    const parentLabel = parent?.dimension
      ? dimensionDisplay(parent.dimension, parent.name, t)
      : dimension;
    return `${parentLabel} · ${classificationValueDisplayName(
      graph.dimensionValue ?? "",
      graph.name,
      locale,
    )}`;
  }
  return dimension;
}

function dimensionDisplay(
  dimension: string,
  fallback: string,
  t: Translator,
) {
  const translated = dimensionLabel(dimension, t);
  return translated === dimension ? fallback : translated;
}

export function dimensionLabel(dimension: string, t: Translator) {
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
    tags: "dimensionTags",
  } as const;
  const key = keys[dimension as keyof typeof keys];
  return key ? t(key) : dimension;
}

function normalizeError(error: unknown) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "GraphSpaces failed to load.";
}
