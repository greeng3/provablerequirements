import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";

import type { LayoutInput, LayoutResult } from "./types";
import { NODE_HEIGHT, NODE_WIDTH } from "./types";

interface SimNode extends SimulationNodeDatum {
  id: string;
}

/// Converges a d3-force simulation to steady state and returns
/// the final positions keyed by node UUID. Iteration count is
/// hard-capped so graphs at the 500-node bound finish inside a
/// few hundred ms on mid-range hardware.
export function forceLayout(
  input: LayoutInput,
  width = 1200,
  height = 800,
): LayoutResult {
  const nodes: SimNode[] = input.nodes.map((n) => ({ id: n.uuid }));
  const links: SimulationLinkDatum<SimNode>[] = input.edges.map((e) => ({
    source: e.sourceUuid,
    target: e.targetUuid,
  }));

  const simulation = forceSimulation<SimNode>(nodes)
    .force(
      "link",
      forceLink<SimNode, SimulationLinkDatum<SimNode>>(links)
        .id((d) => d.id)
        .distance(140)
        .strength(0.5),
    )
    .force("charge", forceManyBody().strength(-320))
    .force("center", forceCenter(width / 2, height / 2))
    .force("collide", forceCollide(Math.max(NODE_WIDTH, NODE_HEIGHT) / 2 + 8))
    .stop();

  // d3-force's recommended tick count for steady-state layout.
  for (let i = 0; i < 300; i += 1) simulation.tick();

  return nodes.map((n) => ({
    id: n.id,
    x: (n.x ?? 0) - NODE_WIDTH / 2,
    y: (n.y ?? 0) - NODE_HEIGHT / 2,
  }));
}
