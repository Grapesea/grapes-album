import { useEffect, useMemo, useRef, useState } from "react";
import Graph from "graphology";
import FA2Layout from "graphology-layout-forceatlas2/worker";
import {
  SigmaContainer,
  useRegisterEvents,
  useSigma,
} from "@react-sigma/core";
import { Pin, PinOff } from "lucide-react";
import type { GraphNode, GraphView } from "../bindings/types";
import { useI18n } from "../i18n";

interface SavedPosition {
  nodeKey: string;
  x: number;
  y: number;
  pinned: boolean;
}

interface SigmaGraphProps {
  view: GraphView;
  layoutNonce: number;
  onNavigate: (graphId: string) => void;
  onSelectAsset: (assetId: string) => void;
  onOpenAsset: (assetId: string) => void;
  onSavePositions: (positions: SavedPosition[]) => Promise<void>;
}

export function SigmaGraph({
  view,
  layoutNonce,
  onNavigate,
  onSelectAsset,
  onOpenAsset,
  onSavePositions,
}: SigmaGraphProps) {
  const { t } = useI18n();
  const [hovered, setHovered] = useState<GraphNode | null>(null);
  const graph = useMemo(
    () => createGraphologyGraph(view, (node) => displayNodeLabel(node, t)),
    [t, view],
  );
  const [pinnedNodes, setPinnedNodes] = useState<Set<string>>(
    () => new Set(view.nodes.filter((node) => node.pinned).map((node) => node.id)),
  );

  useEffect(() => {
    setPinnedNodes(
      new Set(view.nodes.filter((node) => node.pinned).map((node) => node.id)),
    );
  }, [view]);

  const togglePin = async (node: GraphNode) => {
    if (!graph.hasNode(node.id)) return;
    const pinned = !graph.getNodeAttribute(node.id, "fixed");
    graph.setNodeAttribute(node.id, "fixed", pinned);
    setPinnedNodes((current) => {
      const next = new Set(current);
      if (pinned) next.add(node.id);
      else next.delete(node.id);
      return next;
    });
    await onSavePositions([
      {
        nodeKey: node.id,
        x: graph.getNodeAttribute(node.id, "x"),
        y: graph.getNodeAttribute(node.id, "y"),
        pinned,
      },
    ]);
  };

  return (
    <div className="sigma-graph-shell">
      <SigmaContainer
        graph={graph}
        className="sigma-graph"
        options={{
          settings: {
            renderEdgeLabels: false,
            enableEdgeEvents: false,
            enableNodeDrag: true,
            dragPositionToAttributes: (position) => ({
              x: position.x,
              y: position.y,
            }),
            labelDensity: 0.25,
            labelGridCellSize: 90,
            labelRenderedSizeThreshold: 1,
            minCameraRatio: 0.08,
            maxCameraRatio: 8,
          },
          nodeReducer: (node, attributes) => {
            const data = view.nodes.find((item) => item.id === node);
            if (!data) return attributes;
            if (!data.available) {
              return {
                ...attributes,
                color: "#5d5563",
              };
            }
            if (data.kind === "asset") {
              return {
                ...attributes,
                color: pinnedNodes.has(node)
                  ? "#e5c878"
                  : hovered?.id === node
                    ? "#d6c7ff"
                    : "#7fcfaa",
                size: hovered?.id === node ? 10 : 7,
                label: "",
              };
            }
            return {
              ...attributes,
              color: pinnedNodes.has(node)
                ? "#e5c878"
                : hovered?.id === node
                  ? "#d8c8ff"
                  : data.id === "portal:graphspaces"
                    ? "#b99aff"
                    : "#aa8aff",
              size: hovered?.id === node ? 24 : 18,
            };
          },
        }}
      >
        <GraphEvents
          view={view}
          graph={graph}
          layoutNonce={layoutNonce}
          onHover={setHovered}
          onNavigate={onNavigate}
          onSelectAsset={onSelectAsset}
          onOpenAsset={onOpenAsset}
          onSavePositions={onSavePositions}
        />
      </SigmaContainer>
      {hovered && (
        <div className="graph-hover-card">
          <strong>{displayNodeLabel(hovered, t)}</strong>
          {hovered.kind === "portal" && hovered.photoCount > 0 ? (
            <span>{t("photoCount", { count: hovered.photoCount })}</span>
          ) : (
            hovered.subtitle && <span>{hovered.subtitle}</span>
          )}
          {!hovered.available && <small>P3</small>}
          <button
            type="button"
            className="graph-pin-button"
            onClick={() => void togglePin(hovered)}
          >
            {pinnedNodes.has(hovered.id) ? (
              <PinOff size={11} />
            ) : (
              <Pin size={11} />
            )}
            {pinnedNodes.has(hovered.id) ? "Unpin" : "Pin"}
          </button>
        </div>
      )}
    </div>
  );
}

function GraphEvents({
  view,
  graph,
  layoutNonce,
  onHover,
  onNavigate,
  onSelectAsset,
  onOpenAsset,
  onSavePositions,
}: SigmaGraphProps & {
  graph: Graph;
  onHover: (node: GraphNode | null) => void;
}) {
  const sigma = useSigma();
  const registerEvents = useRegisterEvents();
  const layoutRef = useRef<FA2Layout | null>(null);

  useEffect(() => {
    const hasSavedLayout = view.nodes.every(
      (node) => node.x != null && node.y != null,
    );
    if (hasSavedLayout && layoutNonce === 0) return;

    const layout = new FA2Layout(graph, {
      settings: {
        gravity: 1,
        scalingRatio: 12,
        slowDown: 1,
        barnesHutOptimize: graph.order > 80,
      },
    });
    layoutRef.current = layout;
    layout.start();
    const timeout = window.setTimeout(() => {
      layout.stop();
      const positions = graph.mapNodes((node) => ({
        nodeKey: node,
        x: graph.getNodeAttribute(node, "x"),
        y: graph.getNodeAttribute(node, "y"),
        pinned: Boolean(graph.getNodeAttribute(node, "fixed")),
      }));
      void onSavePositions(positions);
    }, 1200);

    return () => {
      window.clearTimeout(timeout);
      layout.kill();
      if (layoutRef.current === layout) layoutRef.current = null;
    };
  }, [graph, layoutNonce, onSavePositions, view]);

  useEffect(() => {
    registerEvents({
      enterNode: ({ node }) => {
        onHover(view.nodes.find((item) => item.id === node) ?? null);
      },
      leaveNode: () => onHover(null),
      clickNode: ({ node }) => {
        const selected = view.nodes.find((item) => item.id === node);
        if (selected?.kind === "portal" && selected.targetGraphId) {
          onNavigate(selected.targetGraphId);
        } else if (selected?.kind === "asset" && selected.assetId) {
          onSelectAsset(selected.assetId);
        }
      },
      doubleClickNode: ({ node }) => {
        const selected = view.nodes.find((item) => item.id === node);
        if (selected?.kind === "asset" && selected.assetId) {
          onOpenAsset(selected.assetId);
        }
      },
      nodeDragStart: () => {
        layoutRef.current?.stop();
      },
      nodeDragEnd: ({ node }) => {
        const position: SavedPosition = {
          nodeKey: node,
          x: graph.getNodeAttribute(node, "x"),
          y: graph.getNodeAttribute(node, "y"),
          pinned: Boolean(graph.getNodeAttribute(node, "fixed")),
        };
        void onSavePositions([position]);
      },
    });
  }, [
    graph,
    onHover,
    onNavigate,
    onOpenAsset,
    onSavePositions,
    onSelectAsset,
    registerEvents,
    view,
  ]);

  useEffect(() => {
    sigma.getCamera().animate(
      { x: 0.5, y: 0.5, ratio: 1.15 },
      { duration: 450 },
    );
  }, [sigma, view.graph.id]);

  return null;
}

function createGraphologyGraph(
  view: GraphView,
  getLabel: (node: GraphNode) => string,
) {
  const graph = new Graph();
  const portals = view.nodes.filter((node) => node.kind === "portal");
  const assets = view.nodes.filter((node) => node.kind === "asset");
  const center = portals[0];
  const centerX = center?.x ?? 0;
  const centerY = center?.y ?? 0;
  let portalIndex = 1;
  let assetIndex = 0;

  for (const node of view.nodes) {
    let x = node.x;
    let y = node.y;
    if (x == null || y == null) {
      if (node.id === center?.id) {
        x = 0;
        y = 0;
      } else if (node.kind === "asset") {
        const angle = (assetIndex / Math.max(assets.length, 1)) * Math.PI * 2;
        const radius = Math.max(
          4,
          Math.min(12, 2.8 + Math.sqrt(assets.length) * 0.45),
        );
        x = centerX + Math.cos(angle) * radius;
        y = centerY + Math.sin(angle) * radius;
        assetIndex += 1;
      } else {
        const angle =
          (portalIndex / Math.max(portals.length - 1, 1)) * Math.PI * 2;
        x = centerX + Math.cos(angle) * 3.1;
        y = centerY + Math.sin(angle) * 3.1;
        portalIndex += 1;
      }
    }

    graph.addNode(node.id, {
      x,
      y,
      label: getLabel(node),
      size:
        node.kind === "asset"
          ? 7
          : node.id === "portal:graphspaces"
            ? 22
            : 17,
      color: node.available ? "#aa8aff" : "#5d5563",
      nodeKind: node.kind,
      fixed: node.pinned,
    });
  }

  for (const edge of view.edges) {
    if (
      graph.hasNode(edge.source) &&
      graph.hasNode(edge.target) &&
      !graph.hasEdge(edge.source, edge.target)
    ) {
      graph.addEdge(edge.source, edge.target, {
        size: 1.2,
        color: "rgba(184, 167, 199, 0.28)",
      });
    }
  }
  return graph;
}

function displayNodeLabel(
  node: GraphNode,
  t: ReturnType<typeof useI18n>["t"],
) {
  const id = node.id;
  const dimension = id.startsWith("portal:graph:dimension:")
    ? id.slice("portal:graph:dimension:".length)
    : id.startsWith("portal:") && !id.includes(":graph:")
      ? id.slice("portal:".length)
      : "";
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
  return key ? t(key) : node.label;
}

