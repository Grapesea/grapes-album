import Graph from "graphology";
import type { GraphNode, GraphView } from "../bindings/types";

export const LAYOUT_VERSION = "grape-bunch-v1";
const SPACING = 42;

export interface SavedPosition {
  nodeKey: string;
  x: number;
  y: number;
  pinned: boolean;
}

export function nodeDestination(node: GraphNode) {
  if (node.dummy || !node.available) return null;
  if (node.kind === "portal" && node.targetGraphId) {
    return { kind: "graph" as const, id: node.targetGraphId };
  }
  if (node.kind === "asset" && node.assetId) {
    return { kind: "asset" as const, id: node.assetId };
  }
  return null;
}

// Staggered, tapering rows form the bunch. These anchors constrain ForceAtlas2,
// whose unconstrained equilibrium for a portal graph would otherwise be a star.
export function grapeAnchors(nodes: GraphNode[]) {
  const anchors = new Map<string, { x: number; y: number }>();
  const contents = nodes.filter((node) => !node.dummy);
  const rows = Math.ceil((Math.sqrt(8 * contents.length + 1) - 1) / 2);
  let excess = (rows * (rows + 1)) / 2 - contents.length;
  let index = 0;
  for (let row = 0; row < rows; row++) {
    const count = rows - row - (excess-- > 0 ? 1 : 0);
    for (let column = 0; column < count; column++) {
      const node = contents[index++];
      const jitter = hashString(node.id);
      anchors.set(node.id, {
        x:
          (column - (count - 1) / 2) * SPACING * 1.15 +
          (row % 2 ? 1 : -1) * SPACING * 0.08 +
          ((jitter % 11) - 5) * 0.5,
        y: -row * SPACING * 0.72 + (((jitter >>> 8) % 11) - 5) * 0.5,
      });
    }
  }
  for (const node of nodes.filter((node) => node.dummy)) {
    anchors.set(node.id, { x: 0, y: SPACING * 0.8 });
  }
  return anchors;
}

export function createGrapeGraph(view: GraphView, resetPositions: boolean) {
  const graph = new Graph();
  graph.setAttribute("interactiveLayout", !resetPositions);
  // Even an empty layer has a root with no navigation destination.
  const root = view.nodes.find((node) => node.dummy);
  const nodes: GraphNode[] = root
    ? view.nodes
    : [
        {
          id: `dummy:${view.graph.id}`,
          kind: "portal",
          dummy: true,
          label: view.graph.name,
          subtitle: null,
          targetGraphId: null,
          assetId: null,
          photoCount: 0,
          available: false,
          x: null,
          y: null,
          pinned: false,
        },
        ...view.nodes,
      ];
  const anchors = grapeAnchors(nodes);
  const rootAnchor = root && anchors.get(root.id);
  const restoreRoot =
    !resetPositions &&
    root?.x != null &&
    root.y != null &&
    Number.isFinite(root.x) &&
    Number.isFinite(root.y);
  if (restoreRoot && rootAnchor) {
    const dx = root.x! - rootAnchor.x;
    const dy = root.y! - rootAnchor.y;
    for (const anchor of anchors.values()) {
      anchor.x += dx;
      anchor.y += dy;
    }
  }
  for (const node of nodes) {
    const anchor = anchors.get(node.id)!;
    const saved =
      (!node.dummy || restoreRoot) &&
      (!resetPositions || node.pinned) &&
      node.x != null &&
      node.y != null &&
      Number.isFinite(node.x) &&
      Number.isFinite(node.y);
    graph.addNode(node.id, {
      x: saved ? node.x : anchor.x,
      y: saved ? node.y : anchor.y,
      anchorX: anchor.x,
      anchorY: anchor.y,
      lockedX: saved ? node.x : anchor.x,
      lockedY: saved ? node.y : anchor.y,
      layoutX: saved ? node.x : anchor.x,
      layoutY: saved ? node.y : anchor.y,
      size: node.dummy ? 3 : node.kind === "asset" ? 5 : 8,
      label: node.dummy ? "" : node.label,
      dummy: node.dummy,
      synthetic: !root && node.id === nodes[0].id,
      fixed: !node.dummy && node.pinned,
    });
  }
  for (const edge of view.edges) {
    if (
      graph.hasNode(edge.source) &&
      graph.hasNode(edge.target) &&
      edge.source !== edge.target &&
      !graph.hasEdge(edge.source, edge.target)
    ) {
      graph.addEdge(edge.source, edge.target, {
        weight: Number.isFinite(edge.weight) ? edge.weight : 0.15,
      });
    }
  }
  const rootId = root?.id ?? nodes[0].id;
  for (const node of nodes) {
    if (!node.dummy && graph.degree(node.id) === 0) {
      graph.addEdge(rootId, node.id, { weight: 0.15 });
    }
  }
  return graph;
}

export function releaseGrapeNode(graph: Graph, key: string) {
  const node = graph.getNodeAttributes(key);
  if (!node.dragging) return;
  graph.setAttribute("interactiveLayout", true);
  graph.mergeNodeAttributes(key, {
    dragging: false,
    fixed: false,
    x: node.dragX,
    y: node.dragY,
    lockedX: node.dragX,
    lockedY: node.dragY,
    layoutX: node.dragX,
    layoutY: node.dragY,
    anchorX: node.dragX,
    anchorY: node.dragY,
  });
}

export function constrainToBunch(
  graph: Graph,
  key: string,
  proposed: Record<string, number>,
  timeStep = 1,
) {
  const current = graph.getNodeAttributes(key);
  const position = (x: number, y: number) => ({
    ...proposed,
    x,
    y,
    layoutX: x,
    layoutY: y,
  });
  if (current.dragging) return position(current.dragX, current.dragY);
  if (current.fixed) return position(current.lockedX, current.lockedY);

  // The worker mutates x/y before calling this reducer. Keep the last displayed
  // position separately so a distant drop cannot snap back in a single update.
  const x = current.layoutX;
  const y = current.layoutY;
  const spring = graph.getAttribute("interactiveLayout") ? 0 : 0.06;
  const dx =
    (Number.isFinite(proposed.x) ? proposed.x - x : 0) +
    (current.anchorX - x) * spring;
  const dy =
    (Number.isFinite(proposed.y) ? proposed.y - y : 0) +
    (current.anchorY - y) * spring;
  // A soft spring preserves the bunch while allowing force-driven displacement.
  // Bound velocity, not position, and pace movement by wall time rather than
  // worker throughput (which varies greatly between devices).
  const scale = Math.min(1, 3 / Math.max(Math.hypot(dx, dy), 0.001)) * timeStep;
  return position(x + dx * scale, y + dy * scale);
}

export function resetGrapePositions(graph: Graph) {
  graph.setAttribute("interactiveLayout", false);
  graph.updateEachNodeAttributes((_key, attributes) =>
    attributes.fixed
      ? attributes
      : {
          ...attributes,
          x: attributes.anchorX,
          y: attributes.anchorY,
          layoutX: attributes.anchorX,
          layoutY: attributes.anchorY,
        },
  );
}

export function savedPositions(graph: Graph): SavedPosition[] {
  return graph
    .filterNodes((_key, attributes) => !attributes.synthetic)
    .map((nodeKey) => ({
      nodeKey,
      x: graph.getNodeAttribute(nodeKey, "x"),
      y: graph.getNodeAttribute(nodeKey, "y"),
      pinned:
        !graph.getNodeAttribute(nodeKey, "dummy") &&
        Boolean(graph.getNodeAttribute(nodeKey, "fixed")),
    }));
}

function hashString(value: string) {
  let hash = 0;
  for (let index = 0; index < value.length; index++) {
    hash = (hash * 31 + value.charCodeAt(index)) >>> 0;
  }
  return hash;
}
