import { useEffect, useState } from "react";

import { ApiError } from "../../api/client";
import { useMigrateProjectSchema } from "../../api/queries";
import type { BulkMigrateResult } from "../../api/types";

interface Props {
  readonly projectSlug: string;
  readonly onClose: () => void;
}

/// Phase 11a migrate-schema modal, mounted from the Project
/// detail page. Confirms with the operator, calls the bulk-
/// migrate endpoint, and shows the per-file outcome. A 409
/// response (dirty worktree) surfaces as a warning banner with
/// an explicit "run anyway" path.
export function MigrateSchemaDialog({ projectSlug, onClose }: Props) {
  const mutation = useMigrateProjectSchema(projectSlug);
  const [result, setResult] = useState<BulkMigrateResult | null>(null);
  const [dirtyWarning, setDirtyWarning] = useState<string | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const run = (force: boolean) => {
    setDirtyWarning(null);
    mutation.mutate(
      { force },
      {
        onSuccess: (resp) => setResult(resp.result),
        onError: (err) => {
          if (err instanceof ApiError && err.status === 409) {
            const body = err.body as { error?: string } | undefined;
            setDirtyWarning(
              body?.error ?? "Project worktree has uncommitted changes.",
            );
          }
        },
      },
    );
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="migrate-schema-heading"
      data-testid="migrate-schema-dialog"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="max-h-[90vh] w-full max-w-2xl overflow-auto rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="migrate-schema-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Migrate this Project to the latest schema
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          Walks every ReqForge-authored file in{" "}
          <span className="font-mono">{projectSlug}</span> and rewrites any file
          whose <code>schemaVersion</code> is below the current. ReqForge never
          commits — stage + commit the rewrites yourself so the migration lands
          as its own change.
        </p>

        {result ? (
          <ResultPanel result={result} />
        ) : (
          <>
            {dirtyWarning ? (
              <div
                role="alert"
                data-testid="migrate-schema-dirty"
                className="mt-4 rounded border border-amber-300 bg-amber-50 p-3 text-sm text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100"
              >
                <p className="font-semibold">
                  The project worktree has uncommitted changes.
                </p>
                <p className="mt-1">
                  Commit them first so the migration lands as its own commit, or
                  proceed anyway and mix them together.
                </p>
                <p className="mt-1 text-xs">{dirtyWarning}</p>
              </div>
            ) : null}
            {mutation.error && !dirtyWarning ? (
              <p
                className="mt-4 text-sm text-rose-600"
                role="alert"
                data-testid="migrate-schema-error"
              >
                {String(mutation.error)}
              </p>
            ) : null}
            <div className="mt-6 flex flex-wrap justify-end gap-2">
              <button
                type="button"
                onClick={onClose}
                className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
              >
                Cancel
              </button>
              {dirtyWarning ? (
                <button
                  type="button"
                  onClick={() => run(true)}
                  disabled={mutation.isPending}
                  data-testid="migrate-schema-force"
                  className="rounded bg-amber-600 px-3 py-1 text-sm text-white hover:bg-amber-700 disabled:opacity-50"
                >
                  {mutation.isPending ? "Running…" : "Run anyway"}
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => run(false)}
                  disabled={mutation.isPending}
                  data-testid="migrate-schema-run"
                  className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
                >
                  {mutation.isPending ? "Running…" : "Run migration"}
                </button>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

interface ResultPanelProps {
  readonly result: BulkMigrateResult;
}

function ResultPanel({ result }: ResultPanelProps) {
  const everythingCurrent =
    result.filesRewritten === 0 && result.failures.length === 0;
  return (
    <div className="mt-4 space-y-3" data-testid="migrate-schema-result">
      {everythingCurrent ? (
        <div className="rounded border border-emerald-300 bg-emerald-50 p-3 text-sm text-emerald-900 dark:border-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-100">
          Every one of the {result.filesScanned} ReqForge-authored file
          {result.filesScanned === 1 ? "" : "s"} is already at the current
          schema. Nothing was rewritten.
        </div>
      ) : (
        <dl className="grid grid-cols-2 gap-2 text-sm">
          <Stat label="Scanned" value={result.filesScanned} />
          <Stat label="Rewritten" value={result.filesRewritten} />
          <Stat label="Up to date" value={result.filesUpToDate} />
          <Stat label="Failures" value={result.failures.length} />
        </dl>
      )}

      {result.rewritten.length > 0 ? (
        <details
          className="rounded border border-slate-200 p-3 text-xs dark:border-slate-700"
          data-testid="migrate-schema-rewritten"
        >
          <summary className="cursor-pointer font-semibold">
            {result.rewritten.length} file
            {result.rewritten.length === 1 ? "" : "s"} rewritten
          </summary>
          <ul className="mt-2 space-y-1 font-mono">
            {result.rewritten.map((r, idx) => (
              <li key={`${r.path}-${idx}`}>
                {r.path} · v{r.outcome.fromVersion} → v{r.outcome.toVersion}
              </li>
            ))}
          </ul>
        </details>
      ) : null}

      {result.failures.length > 0 ? (
        <details
          open
          className="rounded border border-rose-300 bg-rose-50 p-3 text-xs text-rose-900 dark:border-rose-700 dark:bg-rose-900/30 dark:text-rose-100"
          data-testid="migrate-schema-failures"
        >
          <summary className="cursor-pointer font-semibold">
            {result.failures.length} failure
            {result.failures.length === 1 ? "" : "s"}
          </summary>
          <ul className="mt-2 space-y-1 font-mono">
            {result.failures.map((f, idx) => (
              <li key={`${f.path}-${idx}`}>
                {f.path} · <span className="italic">{f.fileType}</span>:{" "}
                {f.error}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </div>
  );
}

function Stat({
  label,
  value,
}: {
  readonly label: string;
  readonly value: number;
}) {
  return (
    <div className="rounded border border-slate-200 p-2 dark:border-slate-700">
      <dt className="text-xs uppercase tracking-wide text-slate-500">
        {label}
      </dt>
      <dd className="text-lg font-semibold">{value}</dd>
    </div>
  );
}
