import type { ReviewLogEntry } from "../../api/types";

/// Compact review-state summary: total entries + the most recent
/// outcome. Full review management lands in Phase 4.
export function ReviewSummary({ reviewLog }: { reviewLog: ReviewLogEntry[] }) {
  if (reviewLog.length === 0) {
    return <p className="text-sm text-slate-500">No review entries.</p>;
  }

  const last = reviewLog[reviewLog.length - 1];
  const label = reviewLog.length === 1 ? "entry" : "entries";

  return (
    <p className="text-sm text-slate-600 dark:text-slate-400">
      <span className="font-semibold text-slate-900 dark:text-slate-100">
        {reviewLog.length}
      </span>{" "}
      review {label} · latest: <span className="font-mono">{last.outcome}</span>
      {last.reviewer ? ` by ${last.reviewer}` : ""}
    </p>
  );
}
