import dagre from "dagre";

import type { LayoutInput, LayoutResult } from "./types";
import { NODE_HEIGHT, NODE_WIDTH } from "./types";

/// Top-to-bottom hierarchical layout via dagre. Only acyclic
/// edges feed the layout — a cyclic edge would add a back-edge
/// that dagre tries to reverse internally, which confuses the
/// user's mental model ("why is my satisfies edge pointing up?").
/// Non-acyclic edges still appear on the canvas, just not as
/// layout signal.
export function dagreLayout(input: LayoutInput): LayoutResult {
  const graph = new dagre.graphlib.Graph();
  graph.setDefaultEdgeLabel(() => ({}));
  graph.setGraph({
    rankdir: "TB",
    nodesep: 40,
    ranksep: 80,
  });

  for (const node of input.nodes) {
    graph.setNode(node.uuid, { width: NODE_WIDTH, height: NODE_HEIGHT });
  }
  for (const edge of input.edges) {
    if (!edge.acyclic) continue;
    graph.setEdge(edge.sourceUuid, edge.targetUuid);
  }

  dagre.layout(graph);

  return input.nodes.map((n) => {
    const pos = graph.node(n.uuid);
    // Dagre returns centre coordinates; React Flow positions
    // nodes by their top-left, so we offset half the sizing.
    return {
      id: n.uuid,
      x: (pos?.x ?? 0) - NODE_WIDTH / 2,
      y: (pos?.y ?? 0) - NODE_HEIGHT / 2,
    };
  });
}
