import { useState } from "react";
import { Link } from "react-router-dom";

import { useReport } from "../../api/queries";
import type {
  CyclesReportPayload,
  ReportGraphNode,
  ReportScopeParam,
} from "../../api/types";

import { ReportHeader } from "./ReportHeader";

export function CyclesReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const report = useReport("cycles", scope, includeInactive);

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="cycles"
        title="Cycles"
        description="Cycles in link types that are expected to be acyclic (derives-from, supersedes). Each is a modelling error to investigate."
        scope={scope}
        includeInactive={includeInactive}
        onScopeChange={setScope}
        onIncludeInactiveChange={setIncludeInactive}
        onResetToDefaults={() => {
          setScope("system");
          setIncludeInactive(false);
        }}
      />

      {report.isLoading ? (
        <p className="text-sm text-slate-500">Loading…</p>
      ) : report.isError || !report.data ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load report: {String(report.error ?? "unknown")}
        </p>
      ) : report.data.kind !== "cycles" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body report={report.data} />
      )}
    </section>
  );
}

function Body({ report }: { report: CyclesReportPayload }) {
  return (
    <div className="space-y-3">
      <p className="text-xs text-slate-500">
        Checked link types:{" "}
        {report.linkTypesChecked.map((t) => (
          <span
            key={t}
            className="mr-1 rounded bg-slate-100 px-1.5 py-0.5 font-mono text-slate-700 dark:bg-slate-800 dark:text-slate-200"
          >
            {t}
          </span>
        ))}
      </p>
      {report.truncated ? (
        <p
          role="alert"
          className="rounded border border-amber-300 bg-amber-50 p-2 text-xs text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100"
        >
          Output truncated — at least one link type hit the per-type cap.
          Resolve these cycles and reload to see the next batch.
        </p>
      ) : null}

      {report.totalCycles === 0 ? (
        <p className="text-sm text-slate-500">No cycles in scope. 🎉</p>
      ) : (
        <ul className="space-y-3">
          {report.cycles.map((cycle, idx) => (
            <li
              key={`${cycle.linkType}-${idx}`}
              className="rounded border border-slate-200 p-3 text-sm dark:border-slate-800"
            >
              <p className="mb-2 text-xs uppercase tracking-wide text-slate-500">
                <span className="font-mono">{cycle.linkType}</span> ·{" "}
                {cycle.nodes.length} nodes
              </p>
              <CycleChain nodes={cycle.nodes} />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function CycleChain({ nodes }: { nodes: ReportGraphNode[] }) {
  return (
    <p className="flex flex-wrap items-center gap-1 text-xs">
      {nodes.map((n, i) => (
        <span key={n.uuid} className="flex items-center gap-1">
          <NodeLink node={n} />
          <span aria-hidden className="text-slate-400">
            →
          </span>
          {i === nodes.length - 1 ? <NodeLink node={nodes[0]!} faded /> : null}
        </span>
      ))}
    </p>
  );
}

function NodeLink({ node, faded }: { node: ReportGraphNode; faded?: boolean }) {
  return (
    <Link
      to={`/projects/${node.projectSlug}/collections/${node.collectionPrefix}/artifacts/${node.artifactName}`}
      className={`font-mono text-xs underline ${
        faded ? "text-slate-400" : "text-sky-700 dark:text-sky-300"
      }`}
      title={node.title}
    >
      {node.projectSlug}/{node.collectionPrefix}/{node.artifactName}
    </Link>
  );
}
