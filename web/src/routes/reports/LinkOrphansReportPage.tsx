import { useState } from "react";
import { Link } from "react-router-dom";

import { useReport } from "../../api/queries";
import type { LinkOrphansReport, ReportScopeParam } from "../../api/types";

import { ReportHeader } from "./ReportHeader";

export function LinkOrphansReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const report = useReport("link-orphans", scope, includeInactive);

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="link-orphans"
        title="Link-graph orphans"
        description="Artifacts with no incoming or outgoing traceability links."
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
      ) : report.data.kind !== "link-orphans" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body report={report.data} />
      )}
    </section>
  );
}

function Body({ report }: { report: LinkOrphansReport }) {
  if (report.totalOrphans === 0) {
    return (
      <p className="text-sm text-slate-500">
        No link-graph orphans in scope. 🎉
      </p>
    );
  }
  return (
    <div className="space-y-3">
      <p className="text-sm text-slate-600 dark:text-slate-400">
        {report.totalOrphans} orphan artifact
        {report.totalOrphans === 1 ? "" : "s"} in scope.
      </p>
      <div className="overflow-auto rounded border border-slate-200 dark:border-slate-800">
        <table
          className="w-full border-collapse text-sm"
          aria-label="Link-graph orphans"
        >
          <thead className="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500 dark:bg-slate-900">
            <tr>
              <th className="p-2">Artifact</th>
              <th className="p-2">Shape</th>
              <th className="p-2">Status</th>
            </tr>
          </thead>
          <tbody>
            {report.entries.map((e) => (
              <tr
                key={e.uuid}
                className="border-t border-slate-200 dark:border-slate-800"
              >
                <td className="p-2">
                  <Link
                    to={`/projects/${e.projectSlug}/collections/${e.collectionPrefix}/artifacts/${e.artifactName}`}
                    className="font-mono text-xs text-sky-700 underline dark:text-sky-300"
                  >
                    {e.projectSlug}/{e.collectionPrefix}/{e.artifactName}
                  </Link>
                  <span className="ml-2 text-slate-500">{e.title}</span>
                </td>
                <td className="p-2 font-mono text-xs">{e.shape}</td>
                <td className="p-2 text-xs">
                  {!e.active ? (
                    <span className="mr-1 rounded bg-slate-200 px-1.5 py-0.5 text-slate-700 dark:bg-slate-700 dark:text-slate-200">
                      inactive
                    </span>
                  ) : null}
                  {e.derived ? (
                    <span className="rounded bg-amber-100 px-1.5 py-0.5 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200">
                      derived
                    </span>
                  ) : null}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
