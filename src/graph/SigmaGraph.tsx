import { useEffect, useMemo, useState } from "react";
import Graph from "graphology";
import {
  SigmaContainer,
  useRegisterEvents,
  useSigma,
} from "@react-sigma/core";
import type { GraphNode, GraphView } from "../bindings/types";

interface SigmaGraphProps {
  view: GraphView;
  onNavigate: (graphId: string) => void;
  onSelectAsset: (assetId: string) => void;
  onOpenAsset: (assetId: string) => void;
}

export function SigmaGraph({
  view,
  onNavigate,
  onSelectAsset,
  onOpenAsset,
}: SigmaGraphProps) {
  const [hovered, setHovered] = useState<GraphNode | null>(null);
  const graph = useMemo(() => createGraphologyGraph(view), [view]);

  return (
    <div className="sigma-graph-shell">
      <SigmaContainer
        graph={graph}
        className="sigma-graph"
        options={{
          settings: {
            renderEdgeLabels: false,
            enableEdgeEvents: false,
            enableNodeDrag: false,
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
                label: "",
              };
            }
            if (data.kind === "asset") {
              return {
                ...attributes,
                color: hovered?.id === node ? "#d6c7ff" : "#7fcfaa",
                size: hovered?.id === node ? 10 : 7,
                label: "",
              };
            }
            return {
              ...attributes,
              color: hovered?.id === node ? "#d8c8ff" : data.id === "portal:graphspaces"
                ? "#b99aff"
                : "#aa8aff",
              size: hovered?.id === node ? 24 : 18,
            };
          },
        }}
      >
        <GraphEvents
          view={view}
          onHover={setHovered}
          onNavigate={onNavigate}
          onSelectAsset={onSelectAsset}
          onOpenAsset={onOpenAsset}
        />
      </SigmaContainer>
      {hovered && (
        <div className="graph-hover-card">
          <strong>{hovered.label}</strong>
          {hovered.subtitle && <span>{hovered.subtitle}</span>}
          {!hovered.available && <small>P3</small>}
        </div>
      )}
    </div>
  );
}

function GraphEvents({
  view,
  onHover,
  onNavigate,
  onSelectAsset,
  onOpenAsset,
}: SigmaGraphProps & {
  onHover: (node: GraphNode | null) => void;
}) {
  const sigma = useSigma();
  const registerEvents = useRegisterEvents();

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
    });
  }, [onHover, onNavigate, onOpenAsset, onSelectAsset, registerEvents, view]);

  useEffect(() => {
    sigma.getCamera().animate(
      { x: 0.5, y: 0.5, ratio: 1.15 },
      { duration: 450 },
    );
  }, [sigma, view.graph.id]);

  return null;
}

function createGraphologyGraph(view: GraphView) {
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
      label: node.label,
      size:
        node.kind === "asset"
          ? 7
          : node.id === "portal:graphspaces"
            ? 22
            : 17,
      color: node.available ? "#aa8aff" : "#5d5563",
      nodeKind: node.kind,
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

