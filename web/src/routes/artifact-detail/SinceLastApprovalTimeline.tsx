import type { OpenTodo, ReviewLogEntry } from "../../api/types";

/// Chronological activity timeline from the last approval to now,
/// per `UX-reviewPane`. Resolved TODOs strike through; unresolved
/// TODOs carry a pending indicator.
export function SinceLastApprovalTimeline({
  log,
  lastApprovalAt,
  blockingTodos,
}: {
  log: ReviewLogEntry[];
  lastApprovalAt: string | null;
  blockingTodos: OpenTodo[];
}) {
  const window = lastApprovalAt
    ? log.filter(
        (e) => e.timestamp >= lastApprovalAt && e.timestamp !== lastApprovalAt,
      )
    : log;
  const openIds = new Set(blockingTodos.map((t) => t.id));

  if (window.length === 0) {
    return (
      <p className="text-sm text-slate-500">
        No activity since the last approval yet.
      </p>
    );
  }

  return (
    <ol className="space-y-2" aria-label="Review activity timeline">
      {window.map((entry, i) => (
        <li
          key={`${entry.timestamp}:${i}`}
          className="rounded border border-slate-200 bg-white p-2 text-sm dark:border-slate-700 dark:bg-slate-900"
        >
          <div className="flex items-baseline justify-between gap-2">
            <span className="font-medium text-slate-800 dark:text-slate-200">
              {entry.outcome}
            </span>
            <span className="font-mono text-xs text-slate-500">
              {entry.timestamp}
            </span>
          </div>
          <p className="text-xs text-slate-500">by {entry.reviewer}</p>
          {entry.explanation && (
            <p className="mt-1 text-slate-700 dark:text-slate-300">
              {entry.explanation}
            </p>
          )}
          {entry.addedTodos && entry.addedTodos.length > 0 && (
            <ul
              className="mt-1 space-y-0.5 text-xs"
              aria-label="TODOs added by this entry"
            >
              {entry.addedTodos.map((t) => {
                const open = openIds.has(t.id);
                return (
                  <li key={t.id} className="flex items-center gap-1">
                    <span
                      className={
                        open
                          ? "inline-block rounded bg-amber-100 px-1 text-amber-800 dark:bg-amber-900/50 dark:text-amber-100"
                          : "inline-block rounded bg-emerald-100 px-1 text-emerald-800 dark:bg-emerald-900/50 dark:text-emerald-100"
                      }
                      aria-label={open ? "pending TODO" : "resolved TODO"}
                    >
                      {open ? "pending" : "resolved"}
                    </span>
                    <span className={open ? "" : "line-through text-slate-500"}>
                      {t.text}
                    </span>
                  </li>
                );
              })}
            </ul>
          )}
          {entry.resolvedTodos && entry.resolvedTodos.length > 0 && (
            <p className="mt-1 text-xs text-slate-500">
              Resolved: {entry.resolvedTodos.join(", ")}
            </p>
          )}
        </li>
      ))}
    </ol>
  );
}
