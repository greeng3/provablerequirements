import { useState } from "react";
import { Link } from "react-router-dom";

import { useReport } from "../../api/queries";
import type { ReportScopeParam, UnresolvedLinksReport } from "../../api/types";

import { ReportHeader } from "./ReportHeader";

export function UnresolvedLinksReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const report = useReport("unresolved-links", scope, includeInactive);

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="unresolved-links"
        title="Unresolved links"
        description="Links whose target UUID is not currently resolvable against the mounted projects."
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
      ) : report.data.kind !== "unresolved-links" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body report={report.data} />
      )}
    </section>
  );
}

function Body({ report }: { report: UnresolvedLinksReport }) {
  if (report.totalUnresolved === 0) {
    return (
      <p className="text-sm text-slate-500">No unresolved links in scope. 🎉</p>
    );
  }
  return (
    <div className="space-y-3">
      <p className="text-sm text-slate-600 dark:text-slate-400">
        {report.totalUnresolved} unresolved link
        {report.totalUnresolved === 1 ? "" : "s"} in scope.
      </p>
      <div className="overflow-auto rounded border border-slate-200 dark:border-slate-800">
        <table
          className="w-full border-collapse text-sm"
          aria-label="Unresolved links"
        >
          <thead className="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500 dark:bg-slate-900">
            <tr>
              <th className="p-2">Source</th>
              <th className="p-2">Link type</th>
              <th className="p-2">Target hint</th>
              <th className="p-2">Reason</th>
            </tr>
          </thead>
          <tbody>
            {report.entries.map((e) => (
              <tr
                key={`${e.sourceUuid}-${e.targetUuid}`}
                className="border-t border-slate-200 dark:border-slate-800"
              >
                <td className="p-2">
                  <Link
                    to={`/projects/${e.sourceProjectSlug}/collections/${e.sourceCollectionPrefix}/artifacts/${e.sourceArtifactName}`}
                    className="font-mono text-xs text-sky-700 underline dark:text-sky-300"
                  >
                    {e.sourceProjectSlug}/{e.sourceCollectionPrefix}/
                    {e.sourceArtifactName}
                  </Link>
                  <span className="ml-2 text-slate-500">{e.sourceTitle}</span>
                </td>
                <td className="p-2 font-mono text-xs">{e.linkType}</td>
                <td className="p-2 font-mono text-xs text-slate-600 dark:text-slate-300">
                  {e.targetHintProjectSlug}/{e.targetHintCollectionPrefix}/
                  {e.targetHintArtifactName}
                </td>
                <td className="p-2">
                  <ReasonPill reason={e.reason} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function ReasonPill({ reason }: { reason: string }) {
  const classes =
    reason === "mount-missing"
      ? "bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200"
      : reason === "target-missing"
        ? "bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200"
        : "bg-slate-200 text-slate-700 dark:bg-slate-800 dark:text-slate-200";
  return (
    <span className={`inline-block rounded px-2 py-0.5 text-xs ${classes}`}>
      {reason}
    </span>
  );
}
