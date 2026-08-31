import type { GraphEdgeDto, GraphNodeDto } from "../../../../api/types";

/// Common input for both layout engines. React Flow nodes carry
/// the raw DTO as data so click handlers and tooltips can reach
/// it without threading separate props.
export interface LayoutInput {
  readonly nodes: readonly GraphNodeDto[];
  readonly edges: readonly GraphEdgeDto[];
}

export interface PositionedNode {
  id: string;
  x: number;
  y: number;
}

/// A layout engine returns positions keyed by node UUID. The
/// shell then zips them back onto React Flow node objects.
export type LayoutResult = PositionedNode[];

export type LayoutKind = "hierarchical" | "force";

/// Canvas sizing constants shared between layout engines and
/// React Flow node rendering. Kept in one place so the dagre
/// sizing hint matches what the actual DOM node measures.
export const NODE_WIDTH = 180;
export const NODE_HEIGHT = 48;
