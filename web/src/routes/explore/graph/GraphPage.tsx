import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useGraph, useLinkTypes } from "../../../api/queries";
import type {
  GraphEdgeDto,
  GraphNodeDto,
  ReportScopeParam,
} from "../../../api/types";
import { GRAPH_NODE_CAP } from "../../../api/types";

import { GraphCanvas } from "./GraphCanvas";
import { GraphFilters } from "./GraphFilters";
import { GraphToast } from "./GraphToast";
import { LinkCreateDialog } from "./LinkCreateDialog";
import type { LayoutKind } from "./layouts/types";

interface PendingLink {
  readonly source: GraphNodeDto;
  readonly target: GraphNodeDto;
}

interface ToastState {
  readonly message: string;
  readonly tone: "info" | "error";
}

/// Graph canvas page. Read-only in 7a.2; 7a.3 added drag-to-link
/// authoring via React Flow's onConnect → LinkCreateDialog →
/// PUT /api/artifacts/:uuid round-trip. Self-link attempts are
/// intercepted before the dialog opens and surface a toast.
export function GraphPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const [linkTypes, setLinkTypes] = useState<string[]>([]);
  const [tags, setTags] = useState<string[]>([]);
  /// `undefined` means "follow the server hint"; the explicit
  /// overrides flip on once the user clicks a toggle.
  const [layoutOverride, setLayoutOverride] = useState<LayoutKind | undefined>(
    undefined,
  );

  const [pendingLink, setPendingLink] = useState<PendingLink | undefined>(
    undefined,
  );
  const [toast, setToast] = useState<ToastState | undefined>(undefined);

  const query = useGraph({
    scope,
    includeInactive,
    linkTypes: linkTypes.length > 0 ? linkTypes : undefined,
    tags: tags.length > 0 ? tags : undefined,
  });
  const linkTypeCatalog = useLinkTypes();
  const navigate = useNavigate();

  // Derive the effective layout: user override wins; otherwise
  // follow the server's acyclic-hint flag.
  const effectiveLayout: LayoutKind = useMemo(() => {
    if (layoutOverride !== undefined) return layoutOverride;
    return query.data?.hintAllEdgesAcyclic ? "hierarchical" : "force";
  }, [layoutOverride, query.data?.hintAllEdgesAcyclic]);

  // Reset the manual override when the scope changes so the
  // server hint is respected again for the new scope.
  useEffect(() => {
    setLayoutOverride(undefined);
  }, [scope]);

  const onNodeClick = (node: GraphNodeDto) => {
    navigate(
      `/projects/${node.projectSlug}/collections/${node.collectionPrefix}/artifacts/${node.artifactName}`,
    );
  };

  const onConnect = (source: GraphNodeDto, target: GraphNodeDto) => {
    setPendingLink({ source, target });
  };

  const onSelfLinkAttempt = () => {
    setToast({
      message: "Self-links aren't supported.",
      tone: "error",
    });
  };

  const onLinkCreated = (linkType: string) => {
    setToast({
      message: `Link created (${linkType}).`,
      tone: "info",
    });
  };

  return (
    <section className="space-y-4">
      <header className="space-y-1 border-b border-slate-200 pb-3 dark:border-slate-800">
        <h1 className="text-2xl font-semibold tracking-tight">Graph</h1>
        <p className="text-sm text-slate-600 dark:text-slate-400">
          Trace links between artifacts. Click a node to jump to its detail
          page; drag from one node to another to create a new link.
        </p>
      </header>

      <GraphFilters
        scope={scope}
        onScopeChange={setScope}
        includeInactive={includeInactive}
        onIncludeInactiveChange={setIncludeInactive}
        selectedLinkTypes={linkTypes}
        onLinkTypesChange={setLinkTypes}
        selectedTags={tags}
        onTagsChange={setTags}
        layout={effectiveLayout}
        onLayoutChange={setLayoutOverride}
        linkTypeNames={(linkTypeCatalog.data ?? []).map((t) => t.name)}
        nodes={query.data?.nodes ?? []}
      />

      {query.isLoading ? (
        <p className="text-sm text-slate-500">Loading graph…</p>
      ) : query.isError || !query.data ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load graph: {String(query.error ?? "unknown")}
        </p>
      ) : (
        <GraphBody
          nodes={query.data.nodes}
          edges={query.data.edges}
          truncated={query.data.truncated}
          totalNodes={query.data.totalNodes}
          layout={effectiveLayout}
          onNodeClick={onNodeClick}
          onConnect={onConnect}
          onSelfLinkAttempt={onSelfLinkAttempt}
        />
      )}

      {pendingLink ? (
        <LinkCreateDialog
          source={pendingLink.source}
          target={pendingLink.target}
          onClose={() => setPendingLink(undefined)}
          onCreated={onLinkCreated}
        />
      ) : null}

      {toast ? (
        <GraphToast
          message={toast.message}
          tone={toast.tone}
          onDismiss={() => setToast(undefined)}
        />
      ) : null}
    </section>
  );
}

interface BodyProps {
  readonly nodes: GraphNodeDto[];
  readonly edges: GraphEdgeDto[];
  readonly truncated: boolean;
  readonly totalNodes: number;
  readonly layout: LayoutKind;
  readonly onNodeClick: (node: GraphNodeDto) => void;
  readonly onConnect: (source: GraphNodeDto, target: GraphNodeDto) => void;
  readonly onSelfLinkAttempt: () => void;
}

function GraphBody({
  nodes,
  edges,
  truncated,
  totalNodes,
  layout,
  onNodeClick,
  onConnect,
  onSelfLinkAttempt,
}: BodyProps) {
  return (
    <div className="space-y-3">
      {truncated ? (
        <p
          role="alert"
          data-testid="graph-truncation-banner"
          className="rounded border border-amber-300 bg-amber-50 p-2 text-sm text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100"
        >
          Showing {GRAPH_NODE_CAP} of {totalNodes} nodes — apply filters to
          narrow the set.
        </p>
      ) : (
        <p className="text-xs text-slate-500">
          {totalNodes} node{totalNodes === 1 ? "" : "s"} · {edges.length} edge
          {edges.length === 1 ? "" : "s"} · layout: {layout}
        </p>
      )}
      {nodes.length === 0 ? (
        <p className="text-sm text-slate-500">
          No artifacts match the current filters. Try broadening the scope or
          clearing tag / link-type filters.
        </p>
      ) : (
        <GraphCanvas
          nodes={nodes}
          edges={edges}
          layout={layout}
          onNodeClick={onNodeClick}
          onConnect={onConnect}
          onSelfLinkAttempt={onSelfLinkAttempt}
        />
      )}
    </div>
  );
}
