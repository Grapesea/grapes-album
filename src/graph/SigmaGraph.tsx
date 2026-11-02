import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type Graph from "graphology";
import FA2Layout from "graphology-layout-forceatlas2/worker";
import { SigmaContainer, useRegisterEvents, useSigma } from "@react-sigma/core";
import { pathLine } from "sigma/rendering";
import type { SigmaOptions } from "sigma/types";
import {
  Maximize2,
  Minus,
  Pin,
  PinOff,
  Plus,
  Search,
  SlidersHorizontal,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import type {
  Dimension,
  GraphInfo,
  GraphNode,
  GraphView,
  SmartGraphFilter,
} from "../bindings/types";
import {
  classificationValueDisplayName,
  dimensionDisplayName,
} from "../classificationLabels";
import { useI18n } from "../i18n";
import {
  constrainToBunch,
  createGrapeGraph,
  LAYOUT_VERSION,
  releaseGrapeNode,
  nodeDestination,
  resetGrapePositions,
  savedPositions,
} from "./grapeLayout";
import type { SavedPosition } from "./grapeLayout";

interface SigmaGraphProps {
  workspaceId: string;
  view: GraphView;
  layoutNonce: number;
  classificationDimensions: Dimension[];
  smartGraphs: GraphInfo[];
  onNavigate: (graphId: string) => void;
  onOpenAsset: (assetId: string) => void;
  onSavePositions: (positions: SavedPosition[]) => Promise<void>;
  onCreateSmartGraph: (
    name: string,
    filters: SmartGraphFilter[],
  ) => Promise<boolean>;
  onDeleteSmartGraph: (graphId: string) => Promise<boolean>;
}

type GraphColorFamily = "capture" | "classification" | "tag";

const GRAPH_COLORS: Record<
  GraphColorFamily,
  { subgraph: string; nested: string; asset: string; edge: string }
> = {
  capture: {
    subgraph: "#1d565c",
    nested: "#2d747a",
    asset: "#89b7bb",
    edge: "#2b4549",
  },
  classification: {
    subgraph: "#432657",
    nested: "#60417a",
    asset: "#ad90c8",
    edge: "#3c3046",
  },
  tag: {
    subgraph: "#73323b",
    nested: "#8d4852",
    asset: "#c88f96",
    edge: "#563038",
  },
};

const CAPTURE_DIMENSIONS = new Set(["date", "camera", "lens"]);

export function SigmaGraph({
  workspaceId,
  view,
  layoutNonce,
  classificationDimensions,
  smartGraphs,
  onNavigate,
  onOpenAsset,
  onSavePositions,
  onCreateSmartGraph,
  onDeleteSmartGraph,
}: SigmaGraphProps) {
  const { locale, t } = useI18n();
  const [hovered, setHovered] = useState<GraphNode | null>(null);
  const [inspected, setInspected] = useState<GraphNode | null>(null);
  const [controlsOpen, setControlsOpen] = useState(false);
  const [smartOpen, setSmartOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [nodeScale, setNodeScale] = useState(1);
  const [edgeScale, setEdgeScale] = useState(0.7);
  const [labelOpacity, setLabelOpacity] = useState(0.7);
  const [smartGraphName, setSmartGraphName] = useState("");
  const [smartFilters, setSmartFilters] = useState<
    Record<string, string[]>
  >({});
  const [creatingSmartGraph, setCreatingSmartGraph] = useState(false);
  const [deletingSmartGraph, setDeletingSmartGraph] = useState<string | null>(
    null,
  );
  const [, updatePins] = useState(0);
  const layoutStorageKey = `grapes.graph.layout:${workspaceId}:${view.graph.id}`;
  const hasCurrentLayout = useMemo(() => {
    try {
      return localStorage.getItem(layoutStorageKey) === LAYOUT_VERSION;
    } catch {
      return false;
    }
  }, [layoutStorageKey]);
  const graph = useMemo(
    () => createGrapeGraph(view, !hasCurrentLayout),
    [view, hasCurrentLayout],
  );
  const nodeById = useMemo(
    () => new Map(view.nodes.map((node) => [node.id, node])),
    [view],
  );
  const contentNodes = useMemo(
    () => view.nodes.filter((node) => !node.dummy),
    [view],
  );
  const matches = useMemo(
    () =>
      new Set(
        contentNodes
          .filter((node) =>
            `${displayNodeLabel(node, locale)} ${node.subtitle ?? ""}`
              .toLocaleLowerCase()
              .includes(query.trim().toLocaleLowerCase()),
          )
          .map((node) => node.id),
      ),
    [contentNodes, locale, query],
  );

  const appearance = useRef({
    hovered,
    query,
    matches,
    labelOpacity,
    nodeScale,
    edgeScale,
    locale,
  });
  appearance.current = {
    hovered,
    query,
    matches,
    labelOpacity,
    nodeScale,
    edgeScale,
    locale,
  };
  const options = useMemo<SigmaOptions>(
    () => ({
      primitives: {
        edges: { paths: [pathLine()] },
        nodes: {
          label: { font: { family: "LXGW WenKai", size: 12 }, margin: 8 },
        },
      },
      styles: {
        stage: { background: "#1e1e1e" },
        nodes: {
          backdropVisibility: "hidden",
          labelPosition: "below",
          labelColor: { attribute: "labelColor" },
          labelVisibility: { attribute: "labelVisibility" },
          cursor: { attribute: "cursor" },
        },
      },
      settings: {
        renderEdgeLabels: false,
        enableEdgeEvents: false,
        enableNodeDrag: true,
        dragPositionToAttributes: (position) => ({
          x: position.x,
          y: position.y,
          dragX: position.x,
          dragY: position.y,
          layoutX: position.x,
          layoutY: position.y,
        }),
        labelDensity: 0.6,
        labelGridCellSize: 80,
        labelRenderedSizeThreshold: 0,
        minCameraRatio: 0.08,
        maxCameraRatio: 5,
        stagePadding: 65,
        itemSizesReference: "screen",
        autoRescale: "once",
        minEdgeThickness: 0,
        nodePickingPadding: 5,
        enableCameraRotation: false,
      },
      nodeReducer: (key, attributes) => {
        const { hovered, query, matches, labelOpacity, nodeScale, locale } =
          appearance.current;
        const node = nodeById.get(key);
        if (!node || node.dummy)
          return {
            ...attributes,
            color: "#59565f",
            size: 3,
            label: "",
            labelVisibility: "hidden",
            cursor: graph.getNodeAttribute(key, "dragging")
              ? "grabbing"
              : "grab",
          };
        const active = hovered?.id === key;
        const dimmed = query.trim() !== "" && !matches.has(key);
        const available = Boolean(nodeDestination(node));
        return {
          ...attributes,
          label: displayNodeLabel(node, locale),
          labelVisibility:
            labelOpacity === 0 || dimmed
              ? "hidden"
              : active ||
                  view.graph.kind === "root" ||
                  contentNodes.length < 30
                ? "visible"
                : "auto",
          labelColor: `rgba(204, 200, 213, ${active ? 1 : labelOpacity})`,
          color: dimmed
            ? "#3a383e"
            : active
              ? "#eee5ff"
              : !available
                ? "#65616b"
                : graph.getNodeAttribute(key, "fixed")
                  ? "#d9bd82"
                  : graphNodeColor(node, view),
          size:
            (node.kind === "portal" ? 8 : 5.5) *
            nodeScale *
            (active ? 1.25 : 1),
          cursor: available ? "pointer" : "default",
        };
      },
      edgeReducer: (edge, attributes) => {
        const { hovered, edgeScale } = appearance.current;
        const [source, target] = graph.extremities(edge);
        const active = hovered?.id === source || hovered?.id === target;
        const sourceNode = nodeById.get(source);
        const targetNode = nodeById.get(target);
        const edgeNode =
          sourceNode?.dummy && targetNode ? targetNode : (sourceNode ?? targetNode);
        const palette = edgeNode
          ? GRAPH_COLORS[graphFamilyForNode(edgeNode, view)]
          : GRAPH_COLORS.classification;
        return {
          ...attributes,
          color: active
            ? hovered
              ? graphNodeColor(hovered, view)
              : palette.asset
            : palette.edge,
          size: edgeScale === 0 ? 0 : active ? edgeScale + 0.5 : edgeScale,
        };
      },
    }),
    [graph, nodeById, contentNodes, view],
  );

  const openNode = useCallback(
    (node: GraphNode) => {
      const destination = nodeDestination(node);
      if (destination?.kind === "graph") onNavigate(destination.id);
      else if (destination?.kind === "asset") onOpenAsset(destination.id);
    },
    [onNavigate, onOpenAsset],
  );

  const togglePin = async (node: GraphNode) => {
    if (node.dummy || !graph.hasNode(node.id)) return;
    graph.mergeNodeAttributes(node.id, {
      fixed: !graph.getNodeAttribute(node.id, "fixed"),
      lockedX: graph.getNodeAttribute(node.id, "x"),
      lockedY: graph.getNodeAttribute(node.id, "y"),
    });
    updatePins((value) => value + 1);
    await onSavePositions(
      savedPositions(graph).filter((position) => position.nodeKey === node.id),
    ).catch(() => {});
  };

  return (
    <div className="sigma-graph-shell">
      <SigmaContainer graph={graph} className="sigma-graph" options={options}>
        <GraphEvents
          graph={graph}
          view={view}
          layoutNonce={layoutNonce}
          hasCurrentLayout={hasCurrentLayout}
          layoutStorageKey={layoutStorageKey}
          onHover={setHovered}
          onInspect={setInspected}
          onOpenNode={openNode}
          onSavePositions={onSavePositions}
        />
        <GraphAppearance />
        <CameraControls />
      </SigmaContainer>

      <div className="graph-options">
        <div className="graph-options-actions">
          <button
            type="button"
            className={`graph-smart-toggle ${smartOpen ? "active" : ""}`}
            title={t("smartGraph")}
            aria-label={t("smartGraph")}
            aria-expanded={smartOpen}
            onClick={() => {
              setSmartOpen((open) => !open);
              setControlsOpen(false);
            }}
          >
            <Sparkles size={14} />
            {t("smartGraph")}
          </button>
          <button
            type="button"
            className={`graph-options-toggle ${controlsOpen ? "active" : ""}`}
            title={t("graphSettings")}
            aria-label={t("graphSettings")}
            aria-expanded={controlsOpen}
            onClick={() => {
              setControlsOpen((open) => !open);
              setSmartOpen(false);
            }}
          >
            <SlidersHorizontal size={16} />
          </button>
        </div>
        {controlsOpen && (
          <aside
            className="graph-settings-panel"
            aria-label={t("graphSettings")}
          >
            <header>
              <strong>{t("graphSettings")}</strong>
              <button
                type="button"
                title={t("closePanel")}
                aria-label={t("closePanel")}
                onClick={() => setControlsOpen(false)}
              >
                <X size={14} />
              </button>
            </header>
            <label className="graph-search">
              <Search size={14} />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("graphSearch")}
                aria-label={t("graphSearch")}
              />
              {query && (
                <button
                  type="button"
                  aria-label={t("clearSearch")}
                  onClick={() => setQuery("")}
                >
                  <X size={12} />
                </button>
              )}
            </label>
            <div className="graph-setting-fields">
              <label>
                {t("graphLabelOpacity")}
                <output>{Math.round(labelOpacity * 100)}%</output>
                <input
                  aria-label={t("graphLabelOpacity")}
                  type="range"
                  min="0"
                  max="1"
                  step="0.05"
                  value={labelOpacity}
                  onChange={(event) =>
                    setLabelOpacity(Number(event.target.value))
                  }
                />
              </label>
              <label>
                {t("graphNodeSize")}
                <output>{nodeScale.toFixed(1)}</output>
                <input
                  aria-label={t("graphNodeSize")}
                  type="range"
                  min="0.6"
                  max="2"
                  step="0.1"
                  value={nodeScale}
                  onChange={(event) => setNodeScale(Number(event.target.value))}
                />
              </label>
              <label>
                {t("graphEdgeWidth")}
                <output>{edgeScale.toFixed(1)}</output>
                <input
                  aria-label={t("graphEdgeWidth")}
                  type="range"
                  min="0"
                  max="2"
                  step="0.1"
                  value={edgeScale}
                  onChange={(event) => setEdgeScale(Number(event.target.value))}
                />
              </label>
            </div>
            <div className="graph-node-list" aria-label={t("graphNodeList")}>
              {contentNodes
                .filter((node) => matches.has(node.id))
                .slice(0, 100)
                .map((node) => (
                  <button
                    type="button"
                    key={node.id}
                    disabled={!nodeDestination(node)}
                    onClick={() => openNode(node)}
                    onFocus={() => setHovered(node)}
                    onBlur={() => setHovered(null)}
                  >
                    <i
                      className={
                        node.kind === "portal" ? "portal-dot" : "asset-dot"
                      }
                      style={{ background: graphNodeColor(node, view) }}
                    />
                    <span>{displayNodeLabel(node, locale)}</span>
                    <small>{node.available ? node.photoCount : "—"}</small>
                  </button>
                ))}
              {matches.size === 0 && <p>{t("graphNoMatches")}</p>}
              {matches.size > 100 && <p>{t("graphSearchMore")}</p>}
            </div>
          </aside>
        )}
        {smartOpen && (
          <aside
            className="graph-settings-panel graph-smart-panel"
            aria-label={t("smartGraph")}
          >
            <header>
              <strong>{t("smartGraph")}</strong>
              <button
                type="button"
                title={t("closePanel")}
                aria-label={t("closePanel")}
                onClick={() => setSmartOpen(false)}
              >
                <X size={14} />
              </button>
            </header>
            <section className="graph-smart-builder">
              <header>
                <strong>{t("createSmartGraph")}</strong>
                <button
                  type="button"
                  onClick={() => setSmartFilters({})}
                  disabled={Object.keys(smartFilters).length === 0}
                >
                  {t("clearConditions")}
                </button>
              </header>
              <p>{t("smartGraphHint")}</p>
              <input
                className="graph-smart-name"
                value={smartGraphName}
                onChange={(event) => setSmartGraphName(event.target.value)}
                placeholder={t("smartGraphName")}
                aria-label={t("smartGraphName")}
              />
              <div className="graph-smart-values">
                {classificationDimensions.map((dimension) => (
                  <details key={dimension.id}>
                    <summary>
                      {dimensionDisplayName(
                        dimension.kind || dimension.id,
                        dimension.name,
                        locale,
                      )}
                      <small>
                        {(smartFilters[dimension.id] ?? []).length || ""}
                      </small>
                    </summary>
                    {dimension.values.map((value) => (
                      <label key={value.id}>
                        <input
                          type="checkbox"
                          checked={(smartFilters[dimension.id] ?? []).includes(
                            value.id,
                          )}
                          onChange={(event) => {
                            setSmartFilters((current) => {
                              const existing = current[dimension.id] ?? [];
                              const values = event.target.checked
                                ? [...existing, value.id]
                                : existing.filter((id) => id !== value.id);
                              if (values.length === 0) {
                                const next = { ...current };
                                delete next[dimension.id];
                                return next;
                              }
                              return { ...current, [dimension.id]: values };
                            });
                          }}
                        />
                        <span>
                          {classificationValueDisplayName(
                            value.id,
                            value.name,
                            locale,
                          )}
                        </span>
                      </label>
                    ))}
                  </details>
                ))}
              </div>
              <button
                type="button"
                className="graph-create-smart"
                disabled={
                  creatingSmartGraph ||
                  !smartGraphName.trim() ||
                  Object.keys(smartFilters).length === 0
                }
                onClick={async () => {
                  const filters = Object.entries(smartFilters).map(
                    ([dimensionId, valueIds]) => ({
                      dimensionId,
                      valueIds,
                    }),
                  );
                  setCreatingSmartGraph(true);
                  try {
                    const created = await onCreateSmartGraph(
                      smartGraphName.trim(),
                      filters,
                    );
                    if (created) {
                      setSmartGraphName("");
                      setSmartFilters({});
                    }
                  } finally {
                    setCreatingSmartGraph(false);
                  }
                }}
              >
                {t("createSmartGraph")}
              </button>
            </section>
            <section className="graph-smart-list">
              <header>
                <strong>{t("savedSmartGraphs")}</strong>
                <small>{smartGraphs.length}</small>
              </header>
              {smartGraphs.length === 0 ? (
                <p>{t("noSmartGraphs")}</p>
              ) : (
                smartGraphs.map((smartGraph) => (
                  <div className="graph-smart-list-item" key={smartGraph.id}>
                    <button
                      type="button"
                      className={`graph-smart-entry ${
                        view.graph.id === smartGraph.id ? "current" : ""
                      }`}
                      onClick={() => {
                        setSmartOpen(false);
                        onNavigate(smartGraph.id);
                      }}
                    >
                      <span>{smartGraph.name}</span>
                      <small>
                        {t("photoCount", { count: smartGraph.photoCount })}
                      </small>
                    </button>
                    <button
                      type="button"
                      className="graph-smart-delete"
                      title={t("deleteSmartGraph")}
                      aria-label={t("deleteSmartGraph")}
                      disabled={deletingSmartGraph !== null}
                      onClick={async () => {
                        if (
                          !window.confirm(
                            t("deleteSmartGraphConfirm", {
                              name: smartGraph.name,
                            }),
                          )
                        ) {
                          return;
                        }
                        setDeletingSmartGraph(smartGraph.id);
                        try {
                          await onDeleteSmartGraph(smartGraph.id);
                        } finally {
                          setDeletingSmartGraph(null);
                        }
                      }}
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                ))
              )}
            </section>
          </aside>
        )}
      </div>

      {inspected && (
        <div className="graph-hover-card">
          <div className="graph-hover-heading">
            <i
              className={
                inspected.kind === "portal" ? "portal-dot" : "asset-dot"
              }
              style={{ background: graphNodeColor(inspected, view) }}
            />
            <strong>{displayNodeLabel(inspected, locale)}</strong>
            <button
              type="button"
              aria-label={t("closePanel")}
              onClick={() => setInspected(null)}
            >
              <X size={12} />
            </button>
          </div>
          <span>
            {inspected.kind === "portal"
              ? t("photoCount", { count: inspected.photoCount })
              : inspected.subtitle}
          </span>
          {!nodeDestination(inspected) ? (
            <small>{t("graphUnavailable")}</small>
          ) : (
            <div className="graph-hover-actions">
              <button
                type="button"
                className="graph-pin-button"
                onClick={() => openNode(inspected)}
              >
                {t(
                  inspected.kind === "portal" ? "graphEnter" : "graphOpenPhoto",
                )}
              </button>
              <button
                type="button"
                className="graph-pin-button"
                onClick={() => void togglePin(inspected)}
              >
                {graph.getNodeAttribute(inspected.id, "fixed") ? (
                  <PinOff size={12} />
                ) : (
                  <Pin size={12} />
                )}
                {t(
                  graph.getNodeAttribute(inspected.id, "fixed")
                    ? "graphUnpin"
                    : "graphPin",
                )}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function GraphEvents({
  graph,
  view,
  layoutNonce,
  hasCurrentLayout,
  layoutStorageKey,
  onHover,
  onInspect,
  onOpenNode,
  onSavePositions,
}: {
  graph: Graph;
  view: GraphView;
  layoutNonce: number;
  hasCurrentLayout: boolean;
  layoutStorageKey: string;
  onHover: (node: GraphNode | null) => void;
  onInspect: (node: GraphNode | null) => void;
  onOpenNode: (node: GraphNode) => void;
  onSavePositions: SigmaGraphProps["onSavePositions"];
}) {
  const sigma = useSigma();
  const registerEvents = useRegisterEvents();
  const simulationRef = useRef<{ wake: () => void } | null>(null);
  const saveRef = useRef(onSavePositions);
  saveRef.current = onSavePositions;
  const dragEndedAt = useRef(0);
  const nodes = useMemo(
    () => new Map(view.nodes.map((node) => [node.id, node])),
    [view],
  );

  useEffect(() => {
    let layout: FA2Layout | null = null;
    let monitor: number | undefined;
    let disposed = false;
    let dirty = false;
    const pinned = new Map(
      graph.mapNodes((key, attributes) => [key, attributes.fixed]),
    );
    const firstNode = graph.nodes()[0];
    const savePositionsForView = saveRef.current;
    const persist = () => {
      dirty = false;
      void savePositionsForView(savedPositions(graph))
        .then(() => {
          try {
            localStorage.setItem(layoutStorageKey, LAYOUT_VERSION);
          } catch {
            /* Optional cache. */
          }
        })
        .catch(() => {
          /* Workspace reports the persistence error. */
        });
    };
    const stop = () => {
      window.clearInterval(monitor);
      monitor = undefined;
      layout?.kill();
      layout = null;
    };
    const wake = () => {
      if (disposed) return;
      // Replace the worker so an in-flight result from before a drag/pin change
      // cannot overwrite the pointer position or use stale fixed flags.
      stop();
      dirty = true;
      graph.updateEachNodeAttributes((_key, attributes) => ({
        ...attributes,
        layoutX: attributes.x,
        layoutY: attributes.y,
      }));
      if (
        graph.everyNode(
          (_key, attributes) => attributes.fixed && !attributes.dragging,
        )
      ) {
        persist();
        return;
      }
      const startedAt = performance.now();
      let lastBatchAt = startedAt;
      let timeStep = 0;
      let quietSince: number | null = null;
      let previous = new Map(
        graph.mapNodes((key, attributes) => [
          key,
          { x: attributes.x, y: attributes.y },
        ]),
      );
      layout = new FA2Layout(graph, {
        settings: {
          gravity: 0.02,
          scalingRatio: 18,
          slowDown: 8,
          barnesHutOptimize: graph.order > 80,
        },
        outputReducer: (key, attributes) => {
          if (key === firstNode) {
            const now = performance.now();
            const held = graph.someNode((_key, node) => node.dragging);
            // Allow settling first, then gradually cool persistent small jitter.
            const cooling = held
              ? 1
              : Math.min(1, Math.max(0, (6500 - (now - startedAt)) / 2000));
            timeStep = Math.min(1, (now - lastBatchAt) / (1000 / 60)) * cooling;
            lastBatchAt = now;
          }
          return constrainToBunch(graph, key, attributes, timeStep);
        },
      });
      layout.start();
      monitor = window.setInterval(() => {
        const now = performance.now();
        const held = graph.someNode((_key, attributes) => attributes.dragging);
        let maxMovement = 0;
        const next = new Map<string, { x: number; y: number }>();
        graph.forEachNode((key, attributes) => {
          const prior = previous.get(key)!;
          maxMovement = Math.max(
            maxMovement,
            Math.hypot(attributes.x - prior.x, attributes.y - prior.y),
          );
          next.set(key, { x: attributes.x, y: attributes.y });
        });
        previous = next;
        if (!held && maxMovement < 0.35) quietSince ??= now;
        else quietSince = null;
        if (
          !held &&
          ((now - startedAt > 1500 &&
            quietSince !== null &&
            now - quietSince > 600) ||
            now - startedAt >= 6500)
        ) {
          stop();
          persist();
        }
      }, 200);
    };
    simulationRef.current = { wake };
    const onAttributes = ({
      key,
      attributes,
    }: {
      key: string;
      attributes: Record<string, unknown>;
    }) => {
      if (pinned.get(key) === attributes.fixed) return;
      pinned.set(key, attributes.fixed);
      wake();
    };
    graph.on("nodeAttributesUpdated", onAttributes);
    if (layoutNonce > 0) resetGrapePositions(graph);
    if (
      layoutNonce > 0 ||
      !hasCurrentLayout ||
      view.nodes.some(
        (node) => !node.dummy && (node.x == null || node.y == null),
      )
    )
      wake();
    return () => {
      disposed = true;
      stop();
      graph.removeListener("nodeAttributesUpdated", onAttributes);
      simulationRef.current = null;
      graph.updateEachNodeAttributes((_key, attributes) => ({
        ...attributes,
        dragging: false,
      }));
      // Navigation may happen before cooling ends; save the latest visible frame
      // using the callback for this graph, never a later navigation target.
      if (dirty) persist();
    };
  }, [graph, hasCurrentLayout, layoutNonce, layoutStorageKey, view]);

  useEffect(() => {
    registerEvents({
      enterNode: ({ node }) => {
        const selected = nodes.get(node);
        if (!selected || selected.dummy) {
          onHover(null);
          return;
        }
        onHover(selected);
        onInspect(selected);
      },
      leaveNode: () => onHover(null),
      clickStage: () => onInspect(null),
      clickNode: ({ node, preventSigmaDefault }) => {
        preventSigmaDefault();
        if (performance.now() - dragEndedAt.current < 250) return;
        const selected = nodes.get(node);
        if (selected) onOpenNode(selected);
      },
      doubleClickNode: ({ node, preventSigmaDefault }) => {
        preventSigmaDefault();
        if (performance.now() - dragEndedAt.current < 250) return;
        const selected = nodes.get(node);
        if (selected) onOpenNode(selected);
      },
      nodeDragStart: ({ node }) => {
        graph.setAttribute("interactiveLayout", true);
        graph.mergeNodeAttributes(node, {
          dragging: true,
          fixed: false,
          dragX: graph.getNodeAttribute(node, "x"),
          dragY: graph.getNodeAttribute(node, "y"),
        });
        simulationRef.current?.wake();
      },
      nodeDragEnd: ({ node }) => {
        dragEndedAt.current = performance.now();
        if (!graph.getNodeAttribute(node, "dragging")) return;
        releaseGrapeNode(graph, node);
        const selected = nodes.get(node);
        if (selected && !selected.dummy) onInspect({ ...selected });
        simulationRef.current?.wake();
      },
    });
  }, [graph, nodes, onHover, onInspect, onOpenNode, registerEvents]);

  useEffect(() => {
    sigma.getCamera().setState({ x: 0.5, y: 0.5, ratio: 1.12, angle: 0 });
  }, [sigma, layoutNonce]);
  return null;
}

function GraphAppearance() {
  const sigma = useSigma();
  // Reducers read current appearance without rebuilding the WebGL renderer.
  useEffect(() => {
    sigma.refresh();
  });
  return null;
}

function CameraControls() {
  const sigma = useSigma();
  const { t } = useI18n();
  return (
    <div className="graph-camera-controls">
      <button
        type="button"
        title={t("graphZoomIn")}
        aria-label={t("graphZoomIn")}
        onClick={() => void sigma.getCamera().zoomIn({ duration: 180 })}
      >
        <Plus size={16} />
      </button>
      <button
        type="button"
        title={t("graphZoomOut")}
        aria-label={t("graphZoomOut")}
        onClick={() => void sigma.getCamera().zoomOut({ duration: 180 })}
      >
        <Minus size={16} />
      </button>
      <button
        type="button"
        title={t("fit")}
        aria-label={t("fit")}
        onClick={() =>
          void sigma
            .getCamera()
            .animate(
              { x: 0.5, y: 0.5, ratio: 1.12, angle: 0 },
              { duration: 250 },
            )
        }
      >
        <Maximize2 size={15} />
      </button>
    </div>
  );
}

function graphFamilyForDimension(
  dimension: string | null | undefined,
): GraphColorFamily {
  const key = dimension?.split(":").at(-1)?.toLocaleLowerCase() ?? "";
  if (CAPTURE_DIMENSIONS.has(key)) return "capture";
  if (key === "tags" || dimension?.startsWith("graph:tags:")) return "tag";
  return "classification";
}

function graphFamilyForNode(node: GraphNode, view: GraphView) {
  if (node.kind === "asset" || view.graph.kind !== "root") {
    return graphFamilyForDimension(view.graph.dimension);
  }
  return graphFamilyForDimension(node.targetGraphId);
}

function graphNodeColor(node: GraphNode, view: GraphView) {
  const palette = GRAPH_COLORS[graphFamilyForNode(node, view)];
  if (node.kind === "asset") return palette.asset;
  return view.graph.kind === "root" ? palette.subgraph : palette.nested;
}

function displayNodeLabel(
  node: GraphNode,
  locale: ReturnType<typeof useI18n>["locale"],
) {
  const id = node.id;
  const dimension = id.startsWith("portal:graph:dimension:")
    ? id.slice("portal:graph:dimension:".length)
    : id.startsWith("portal:") && !id.includes(":graph:")
      ? id.slice("portal:".length)
      : "";
  if (dimension && /^[a-z-]+$/.test(dimension)) {
    return dimensionDisplayName(dimension, node.label, locale);
  }
  return classificationValueDisplayName("", node.label, locale);
}
