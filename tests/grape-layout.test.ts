import assert from "node:assert/strict";
import { test } from "node:test";
import forceAtlas2 from "graphology-layout-forceatlas2";
import {
  constrainToBunch,
  createGrapeGraph,
  grapeAnchors,
  releaseGrapeNode,
  nodeDestination,
  resetGrapePositions,
  savedPositions,
} from "../src/graph/grapeLayout.ts";
import type { GraphNode, GraphView } from "../src/bindings/types.ts";

function viewWith(count: number): GraphView {
  const root: GraphNode = {
    id: "root",
    kind: "portal",
    dummy: true,
    label: "Root",
    subtitle: null,
    targetGraphId: null,
    assetId: null,
    photoCount: count,
    available: true,
    x: null,
    y: null,
    pinned: false,
  };
  const nodes = [
    root,
    ...Array.from({ length: count }, (_, i) => ({
      ...root,
      id: `node:${i}`,
      dummy: false,
      label: `Node ${i}`,
      targetGraphId: `graph:${i}`,
    })),
  ];
  return {
    graph: {
      id: "graphspaces",
      kind: "root",
      parentId: null,
      name: "GraphSpaces",
      dimension: null,
      dimensionValue: null,
      photoCount: count,
      childCount: count,
    },
    breadcrumbs: [],
    nodes,
    edges: nodes
      .slice(1)
      .map((node) => ({
        id: node.id,
        source: root.id,
        target: node.id,
        weight: 0.15,
      })),
  };
}

test("dummy and unavailable nodes have no destination, even with stale targets", () => {
  const view = viewWith(1);
  assert.equal(
    nodeDestination({ ...view.nodes[0], targetGraphId: "stale" }),
    null,
  );
  assert.equal(nodeDestination({ ...view.nodes[1], available: false }), null);
  assert.deepEqual(nodeDestination(view.nodes[1]), {
    kind: "graph",
    id: "graph:0",
  });
  assert.deepEqual(
    nodeDestination({ ...view.nodes[1], kind: "asset", assetId: "photo" }),
    { kind: "asset", id: "photo" },
  );
});

test("every supported size forms finite, unique, tapering rows below its root", () => {
  for (const count of [0, 1, 2, 3, 9, 10, 50, 1000]) {
    const view = viewWith(count);
    const anchors = grapeAnchors(view.nodes);
    assert.equal(anchors.size, count + 1);
    assert.equal(
      new Set([...anchors.values()].map(({ x, y }) => `${x},${y}`)).size,
      count + 1,
    );
    const rows = new Map<number, number>();
    for (const node of view.nodes.slice(1)) {
      const point = anchors.get(node.id)!;
      assert.ok(Number.isFinite(point.x) && Number.isFinite(point.y));
      assert.ok(point.y < anchors.get("root")!.y);
      const row = Math.round(-point.y / (42 * 0.72));
      rows.set(row, (rows.get(row) ?? 0) + 1);
    }
    const widths = [...rows.values()];
    widths.forEach((width, index) => {
      if (index) assert.ok(width <= widths[index - 1]);
    });
    if (count) assert.equal(widths.at(-1), 1);
  }
});

test("migration and relayout preserve pinned content but reset the root", () => {
  const view = viewWith(3);
  view.nodes.forEach((node) => {
    node.x = 600;
    node.y = 700;
  });
  view.nodes[1].pinned = true;
  const graph = createGrapeGraph(view, true);
  assert.equal(graph.getNodeAttribute("node:0", "x"), 600);
  assert.notEqual(graph.getNodeAttribute("node:1", "x"), 600);
  assert.equal(graph.getNodeAttribute("root", "x"), 0);
  graph.setNodeAttribute("node:1", "x", 999);
  resetGrapePositions(graph);
  assert.notEqual(graph.getNodeAttribute("node:1", "x"), 999);
  assert.equal(graph.getNodeAttribute("node:0", "x"), 600);
  assert.equal(
    savedPositions(graph).find((position) => position.nodeKey === "root")!
      .pinned,
    false,
  );
  assert.equal(savedPositions(graph).length, 4);
  assert.equal(
    createGrapeGraph(view, false).getNodeAttribute("node:1", "x"),
    600,
  );
});

test("dragging the root follows normal node rules without moving the bunch", () => {
  const view = viewWith(9);
  view.nodes[1].pinned = true;
  const graph = createGrapeGraph(view, true);
  const pinnedBefore = { ...graph.getNodeAttributes("node:0") };
  const berryBefore = { ...graph.getNodeAttributes("node:1") };
  const rootBefore = { ...graph.getNodeAttributes("root") };
  const rootX = rootBefore.x + 80;
  const rootY = rootBefore.y - 40;
  graph.mergeNodeAttributes("root", {
    dragging: true,
    dragX: rootX,
    dragY: rootY,
  });
  releaseGrapeNode(graph, "root");
  const berryAfter = graph.getNodeAttributes("node:1");
  assert.equal(berryAfter.anchorX, berryBefore.anchorX);
  assert.equal(berryAfter.anchorY, berryBefore.anchorY);
  assert.equal(
    berryAfter.x,
    berryBefore.x,
    "dragging the root must not teleport the bunch",
  );
  const pinned = constrainToBunch(graph, "node:0", { x: 999, y: 999 });
  assert.equal(pinned.x, pinnedBefore.x);
  assert.equal(pinned.y, pinnedBefore.y);
  const root = graph.getNodeAttributes("root");
  assert.equal(root.x, rootX);
  assert.equal(root.y, rootY);
  assert.equal(root.anchorX, rootX);
  assert.equal(root.anchorY, rootY);
  assert.equal(root.fixed, false);
  assert.equal(nodeDestination(view.nodes[0]), null);
});

test("ForceAtlas2 keeps the bunch below the root and preserves pinned coordinates", () => {
  const view = viewWith(100);
  view.nodes[1] = { ...view.nodes[1], pinned: true, x: 30, y: -110 };
  const graph = createGrapeGraph(view, true);
  forceAtlas2.assign(graph, {
    iterations: 100,
    settings: { scalingRatio: 18, gravity: 0.02, barnesHutOptimize: true },
    outputReducer: (key, attributes) =>
      constrainToBunch(graph, key, attributes),
  });
  assert.equal(graph.getNodeAttribute("node:0", "x"), 30);
  assert.equal(graph.getNodeAttribute("node:0", "y"), -110);
  for (const node of view.nodes.slice(2)) {
    const attributes = graph.getNodeAttributes(node.id);
    assert.ok(Number.isFinite(attributes.x) && Number.isFinite(attributes.y));
    assert.ok(attributes.y < graph.getNodeAttribute("root", "y"));
  }
});

test("missing root and orphan content are connected without persisting synthetic nodes", () => {
  const view = viewWith(2);
  view.nodes.shift();
  const graph = createGrapeGraph(view, true);
  assert.equal(graph.order, 3);
  assert.equal(graph.size, 2);
  assert.equal(savedPositions(graph).length, 2);
});

test("a released node moves smoothly instead of snapping to its anchor", () => {
  const graph = createGrapeGraph(viewWith(9), true);
  graph.mergeNodeAttributes("node:0", {
    x: 300,
    y: -200,
    layoutX: 300,
    layoutY: -200,
  });
  const result = constrainToBunch(graph, "node:0", { x: 295, y: -195 }, 1);
  assert.ok(Math.hypot(result.x - 300, result.y + 200) <= 3.001);
  assert.ok(Math.hypot(result.x - 300, result.y + 200) > 0);
  assert.ok(
    result.x > 290,
    "a far-away drop must not teleport back into the bunch",
  );
});

test("a held node follows the pointer while surrounding nodes respond to its force", () => {
  const graph = createGrapeGraph(viewWith(9), true);
  const control = createGrapeGraph(viewWith(9), true);
  const neighbor = graph.getNodeAttributes("node:1");
  graph.mergeNodeAttributes("node:0", {
    dragging: true,
    dragX: neighbor.x + 2,
    dragY: neighbor.y + 2,
    x: neighbor.x + 2,
    y: neighbor.y + 2,
    layoutX: neighbor.x + 2,
    layoutY: neighbor.y + 2,
  });
  for (let frame = 0; frame < 30; frame++) {
    for (const target of [graph, control]) {
      forceAtlas2.assign(target, {
        iterations: 1,
        settings: { gravity: 0.02, scalingRatio: 18, slowDown: 8 },
        outputReducer: (key, attributes) =>
          constrainToBunch(target, key, attributes),
      });
    }
  }
  assert.equal(
    graph.getNodeAttribute("node:0", "x"),
    graph.getNodeAttribute("node:0", "dragX"),
  );
  assert.equal(
    graph.getNodeAttribute("node:0", "y"),
    graph.getNodeAttribute("node:0", "dragY"),
  );
  const moved = graph.getNodeAttributes("node:1");
  const unperturbed = control.getNodeAttributes("node:1");
  assert.ok(Math.hypot(moved.x - unperturbed.x, moved.y - unperturbed.y) > 1);
  assert.equal(
    savedPositions(graph).find((node) => node.nodeKey === "node:0")!.pinned,
    false,
  );
});


test("released nodes remain free, influence neighbors and preserve the new equilibrium on reload", () => {
  const view = viewWith(9);
  const graph = createGrapeGraph(view, true);
  const target = graph.getNodeAttributes("node:1");
  const neighborBefore = { ...target };
  const x = target.x + 2, y = target.y + 2;
  graph.mergeNodeAttributes("node:0", { dragging: true, dragX: x, dragY: y });
  releaseGrapeNode(graph, "node:0");
  assert.equal(graph.getNodeAttribute("node:0", "fixed"), false);
  // With no resultant force, even a stale original anchor must not pull a node.
  const balanced = constrainToBunch(graph, "node:0", { x, y });
  assert.equal(balanced.x, x);
  assert.equal(balanced.y, y);
  for (let frame = 0; frame < 60; frame++) {
    forceAtlas2.assign(graph, { iterations: 1,
      settings: { gravity: 0.02, scalingRatio: 18, slowDown: 8 },
      outputReducer: (key, attributes) => constrainToBunch(graph, key, attributes),
    });
  }
  assert.ok(Math.hypot(graph.getNodeAttribute("node:0", "x")-x,graph.getNodeAttribute("node:0", "y")-y)>1);
  const neighbor = graph.getNodeAttributes("node:1");
  assert.ok(Math.hypot(neighbor.x-neighborBefore.x, neighbor.y-neighborBefore.y) > 1);
  for (const position of savedPositions(graph)) Object.assign(view.nodes.find(node=>node.id===position.nodeKey)!,position);
  const restored = createGrapeGraph(view, false);
  assert.equal(restored.getNodeAttribute("node:0", "x"), graph.getNodeAttribute("node:0", "x"));
  assert.equal(restored.getNodeAttribute("node:0", "fixed"), false);
  assert.equal(restored.getNodeAttribute("root", "fixed"), false);
  assert.equal(restored.getAttribute("interactiveLayout"),true);
});
