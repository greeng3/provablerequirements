import { describe, expect, it } from "vitest";

import type { GraphEdgeDto, GraphNodeDto } from "../../../../../api/types";
import { dagreLayout } from "../dagre";
import { forceLayout } from "../force";

function node(uuid: string, name: string): GraphNodeDto {
  return {
    uuid,
    projectSlug: "sample",
    collectionPrefix: "REQ",
    artifactName: name,
    title: name,
    shape: "content",
    active: true,
    derived: false,
    tags: [],
  };
}

function edge(
  source: string,
  target: string,
  linkType: string,
  acyclic: boolean,
): GraphEdgeDto {
  return {
    sourceUuid: source,
    targetUuid: target,
    linkType,
    acyclic,
    directed: true,
  };
}

describe("dagreLayout", () => {
  it("places parents above children for acyclic edges", () => {
    const nodes = [node("a", "REQ-a"), node("b", "REQ-b")];
    const edges = [edge("a", "b", "derives-from", true)];
    const positions = dagreLayout({ nodes, edges });
    const byId = Object.fromEntries(positions.map((p) => [p.id, p]));
    // Top-to-bottom layout: parent 'a' has smaller y than child 'b'.
    expect(byId.a.y).toBeLessThan(byId.b.y);
  });

  it("ignores non-acyclic edges for layout signal but still returns every node", () => {
    const nodes = [node("a", "REQ-a"), node("b", "REQ-b"), node("c", "REQ-c")];
    const edges = [edge("a", "b", "satisfies", false)];
    const positions = dagreLayout({ nodes, edges });
    expect(positions).toHaveLength(3);
    // With no acyclic edges, nodes don't get ranked so their y is
    // effectively a single row; we just assert every node is
    // represented.
    const ids = positions.map((p) => p.id).sort();
    expect(ids).toEqual(["a", "b", "c"]);
  });
});

describe("forceLayout", () => {
  it("assigns finite positions to every node after convergence", () => {
    const nodes = [node("a", "REQ-a"), node("b", "REQ-b"), node("c", "REQ-c")];
    const edges = [
      edge("a", "b", "related-to", false),
      edge("b", "c", "related-to", false),
    ];
    const positions = forceLayout({ nodes, edges }, 800, 600);
    expect(positions).toHaveLength(3);
    for (const pos of positions) {
      expect(Number.isFinite(pos.x)).toBe(true);
      expect(Number.isFinite(pos.y)).toBe(true);
    }
    // Nodes should not all collapse to the same point.
    const uniqueX = new Set(positions.map((p) => Math.round(p.x)));
    expect(uniqueX.size).toBeGreaterThan(1);
  });
});
