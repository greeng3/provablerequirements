import { useState } from "react";

import { useReport } from "../../api/queries";
import type {
  ReportScopeParam,
  ReviewStatusCounts,
  ReviewStatusReportPayload,
} from "../../api/types";

import { ReportHeader } from "./ReportHeader";

export function ReviewStatusReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const report = useReport("review-status", scope, includeInactive);

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="review-status"
        title="Review status"
        description="Approved / rejected / re-requested / never-reviewed counts faceted by project, collection, and artifact shape."
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
      ) : report.data.kind !== "review-status" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body report={report.data} />
      )}
    </section>
  );
}

function Body({ report }: { report: ReviewStatusReportPayload }) {
  const total =
    report.totals.approved +
    report.totals.rejected +
    report.totals.reRequested +
    report.totals.neverReviewed;
  if (total === 0) {
    return (
      <p className="text-sm text-slate-500">
        No artifacts in scope. Try broadening the scope.
      </p>
    );
  }
  return (
    <div className="space-y-4">
      <CountsRow label="Totals" counts={report.totals} />
      <Facet title="By shape">
        <CountsRow label="Content" counts={report.byShape.content} />
        <CountsRow label="Blob" counts={report.byShape.blob} />
        <CountsRow label="URL" counts={report.byShape.url} />
      </Facet>
      {report.byProject.length > 0 ? (
        <Facet title="By project">
          {report.byProject.map((p) => (
            <CountsRow
              key={p.projectSlug}
              label={p.projectSlug}
              counts={p.counts}
            />
          ))}
        </Facet>
      ) : null}
      {report.byCollection.length > 0 ? (
        <Facet title="By collection">
          {report.byCollection.map((c) => (
            <CountsRow
              key={`${c.projectSlug}/${c.collectionPrefix}`}
              label={`${c.projectSlug}/${c.collectionPrefix}`}
              counts={c.counts}
            />
          ))}
        </Facet>
      ) : null}
    </div>
  );
}

function Facet({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded border border-slate-200 p-3 dark:border-slate-800">
      <h2 className="mb-2 text-xs uppercase tracking-wide text-slate-500">
        {title}
      </h2>
      <div className="overflow-auto">
        <table className="w-full border-collapse text-sm" aria-label={title}>
          <thead className="text-left text-xs uppercase tracking-wide text-slate-500">
            <tr>
              <th className="p-1.5"></th>
              <th className="p-1.5">Approved</th>
              <th className="p-1.5">Rejected</th>
              <th className="p-1.5">Re-requested</th>
              <th className="p-1.5">Never reviewed</th>
              <th className="p-1.5">Total</th>
            </tr>
          </thead>
          <tbody>{children}</tbody>
        </table>
      </div>
    </div>
  );
}

function CountsRow({
  label,
  counts,
}: {
  label: string;
  counts: ReviewStatusCounts;
}) {
  const total =
    counts.approved +
    counts.rejected +
    counts.reRequested +
    counts.neverReviewed;
  return (
    <tr className="border-t border-slate-200 dark:border-slate-800">
      <td className="p-1.5 font-mono text-xs">{label}</td>
      <td className="p-1.5 text-emerald-700 dark:text-emerald-300">
        {counts.approved}
      </td>
      <td className="p-1.5 text-rose-700 dark:text-rose-300">
        {counts.rejected}
      </td>
      <td className="p-1.5 text-amber-700 dark:text-amber-300">
        {counts.reRequested}
      </td>
      <td className="p-1.5 text-slate-600 dark:text-slate-300">
        {counts.neverReviewed}
      </td>
      <td className="p-1.5 font-semibold">{total}</td>
    </tr>
  );
}
