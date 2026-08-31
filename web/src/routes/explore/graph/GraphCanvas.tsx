import { useMemo } from "react";
import {
  Background,
  Controls,
  MarkerType,
  ReactFlow,
  type Connection,
  type Edge,
  type Node,
  type NodeMouseHandler,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import type { GraphEdgeDto, GraphNodeDto } from "../../../api/types";

import { dagreLayout } from "./layouts/dagre";
import { forceLayout } from "./layouts/force";
import type { LayoutKind } from "./layouts/types";

interface Props {
  readonly nodes: GraphNodeDto[];
  readonly edges: GraphEdgeDto[];
  readonly layout: LayoutKind;
  readonly onNodeClick: (node: GraphNodeDto) => void;
  /// Invoked when the user completes a drag between two nodes.
  /// `source === target` self-links are handled here — the
  /// parent surfaces a toast for that case.
  readonly onConnect: (source: GraphNodeDto, target: GraphNodeDto) => void;
  /// Invoked when the user starts a drag that resolves to a
  /// self-link (same source and target). Gives the parent a
  /// place to render a toast; GraphCanvas itself is too deep in
  /// the React Flow tree to own toast UI.
  readonly onSelfLinkAttempt: () => void;
}

/// React Flow's `Node.data` is typed as `Record<string, unknown>`,
/// so we attach the raw DTO on a single `dto` key rather than
/// spreading its fields. Keeps the type contract loose for the
/// library while giving consumers a typed handle back.
interface NodeData extends Record<string, unknown> {
  dto: GraphNodeDto;
}

/// Thin React Flow wrapper: runs the chosen layout engine over
/// the raw DTOs, maps positions onto React Flow objects, and
/// delegates click-through to the parent.
export function GraphCanvas({
  nodes,
  edges,
  layout,
  onNodeClick,
  onConnect,
  onSelfLinkAttempt,
}: Props) {
  const byUuid = useMemo(() => {
    const map = new Map<string, GraphNodeDto>();
    for (const n of nodes) map.set(n.uuid, n);
    return map;
  }, [nodes]);
  const flowNodes = useMemo<Node<NodeData>[]>(() => {
    const positions =
      layout === "hierarchical"
        ? dagreLayout({ nodes, edges })
        : forceLayout({ nodes, edges });
    const byId = new Map(positions.map((p) => [p.id, p]));
    return nodes.map((n) => {
      const pos = byId.get(n.uuid) ?? { x: 0, y: 0 };
      return {
        id: n.uuid,
        position: { x: pos.x, y: pos.y },
        type: "default",
        data: {
          dto: n,
          label: (
            <div className="flex flex-col gap-0.5 text-left">
              <span className="font-mono text-[10px] text-slate-500">
                {n.projectSlug}/{n.collectionPrefix}/{n.artifactName}
              </span>
              <span className="line-clamp-2">{n.title}</span>
            </div>
          ),
        },
        style: {
          width: 180,
          padding: 6,
          borderRadius: 6,
          border: n.derived
            ? "1px dashed rgb(148 163 184)"
            : "1px solid rgb(148 163 184)",
          background: n.active ? "rgb(248 250 252)" : "rgb(226 232 240)",
          fontSize: 11,
          opacity: n.active ? 1 : 0.75,
        },
      };
    });
  }, [nodes, edges, layout]);

  const flowEdges = useMemo<Edge[]>(
    () =>
      edges.map((e, idx) => ({
        id: `${e.sourceUuid}-${e.targetUuid}-${e.linkType}-${idx}`,
        source: e.sourceUuid,
        target: e.targetUuid,
        label: e.linkType,
        labelStyle: { fontSize: 10, fill: "rgb(100 116 139)" },
        style: {
          stroke: e.acyclic ? "rgb(56 189 248)" : "rgb(148 163 184)",
          strokeDasharray: e.directed ? undefined : "4 4",
        },
        markerEnd: e.directed
          ? { type: MarkerType.ArrowClosed, color: "rgb(56 189 248)" }
          : undefined,
      })),
    [edges],
  );

  const handleClick: NodeMouseHandler = (_event, node) => {
    const data = node.data as NodeData | undefined;
    if (data?.dto) onNodeClick(data.dto);
  };

  const handleConnect = (connection: Connection) => {
    if (!connection.source || !connection.target) return;
    if (connection.source === connection.target) {
      onSelfLinkAttempt();
      return;
    }
    const source = byUuid.get(connection.source);
    const target = byUuid.get(connection.target);
    if (source && target) onConnect(source, target);
  };

  return (
    <div
      className="h-[calc(100vh-20rem)] min-h-[32rem] rounded border border-slate-200 dark:border-slate-800"
      data-testid="graph-canvas"
    >
      <ReactFlow
        nodes={flowNodes}
        edges={flowEdges}
        onNodeClick={handleClick}
        onConnect={handleConnect}
        fitView
        minZoom={0.2}
        maxZoom={2}
        proOptions={{ hideAttribution: true }}
      >
        <Background gap={16} />
        <Controls />
      </ReactFlow>
    </div>
  );
}
