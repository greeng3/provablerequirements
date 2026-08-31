import { useState } from "react";
import { Link } from "react-router-dom";

import { useReport } from "../../api/queries";
import type {
  ConflictsReportPayload,
  ReportGraphNode,
  ReportScopeParam,
} from "../../api/types";

import { ReportHeader } from "./ReportHeader";

export function ConflictsReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const report = useReport("conflicts", scope, includeInactive);

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="conflicts"
        title="Conflicts"
        description="Pairs of artifacts related by the conflicts-with link type."
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
      ) : report.data.kind !== "conflicts" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body report={report.data} />
      )}
    </section>
  );
}

function Body({ report }: { report: ConflictsReportPayload }) {
  if (report.totalPairs === 0) {
    return (
      <p className="text-sm text-slate-500">No conflict pairs in scope. 🎉</p>
    );
  }
  return (
    <div className="space-y-3">
      <p className="text-sm text-slate-600 dark:text-slate-400">
        {report.totalPairs} conflict pair
        {report.totalPairs === 1 ? "" : "s"} in scope.
      </p>
      <div className="overflow-auto rounded border border-slate-200 dark:border-slate-800">
        <table
          className="w-full border-collapse text-sm"
          aria-label="Conflict pairs"
        >
          <thead className="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500 dark:bg-slate-900">
            <tr>
              <th className="p-2">First</th>
              <th className="p-2">Second</th>
              <th className="p-2">Direction</th>
            </tr>
          </thead>
          <tbody>
            {report.pairs.map((p) => (
              <tr
                key={`${p.first.uuid}-${p.second.uuid}`}
                className="border-t border-slate-200 dark:border-slate-800"
              >
                <td className="p-2">
                  <NodeLink node={p.first} />
                </td>
                <td className="p-2">
                  <NodeLink node={p.second} />
                </td>
                <td className="p-2 text-xs">
                  {p.bidirectional ? (
                    <span className="rounded bg-emerald-100 px-1.5 py-0.5 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200">
                      bidirectional
                    </span>
                  ) : (
                    <span className="rounded bg-amber-100 px-1.5 py-0.5 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200">
                      one-sided
                    </span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function NodeLink({ node }: { node: ReportGraphNode }) {
  return (
    <div>
      <Link
        to={`/projects/${node.projectSlug}/collections/${node.collectionPrefix}/artifacts/${node.artifactName}`}
        className="font-mono text-xs text-sky-700 underline dark:text-sky-300"
      >
        {node.projectSlug}/{node.collectionPrefix}/{node.artifactName}
      </Link>
      <span className="ml-2 text-slate-500">{node.title}</span>
      {!node.active ? (
        <span className="ml-2 rounded bg-slate-200 px-1.5 py-0.5 text-xs text-slate-700 dark:bg-slate-700 dark:text-slate-200">
          inactive
        </span>
      ) : null}
    </div>
  );
}
