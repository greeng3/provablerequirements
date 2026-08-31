import { useEffect, useMemo, useState } from "react";

import { api } from "../../api/client";
import type {
  ArtifactListing,
  BulkRenameSuggestionEntry,
  RenameSuggestion,
} from "../../api/types";

interface Props {
  readonly projectSlug: string;
  readonly collectionPrefixes: readonly string[];
  readonly onClose: () => void;
}

type ArtifactRow = {
  uuid: string;
  currentName: string;
  title: string;
  collectionPrefix: string;
};

type StageState =
  | { kind: "loading" }
  | { kind: "error"; error: string }
  | {
      kind: "results";
      rows: ArtifactRow[];
      entries: BulkRenameSuggestionEntry[];
    };

/// Post-doorstop-import rename wizard per LLM-postImportRenameSuggest.
/// Gathers UUIDs across every collection created by the import,
/// calls the bulk rename-suggestions endpoint, and lets the
/// operator pick one suggestion per row. Applying loops the
/// existing PATCH rename endpoint serially so any individual
/// failure doesn't abort the rest.
export function DoorstopRenameWizard({
  projectSlug,
  collectionPrefixes,
  onClose,
}: Props) {
  const [stage, setStage] = useState<StageState>({ kind: "loading" });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      try {
        const listings = await Promise.all(
          collectionPrefixes.map(async (prefix) => {
            const rows = await api.artifacts(projectSlug, prefix);
            return rows.map<ArtifactRow>((a: ArtifactListing) => ({
              uuid: a.uuid,
              currentName: a.name,
              title: a.title,
              collectionPrefix: prefix,
            }));
          }),
        );
        if (cancelled) return;
        const rows = listings.flat();
        if (rows.length === 0) {
          setStage({
            kind: "error",
            error: "No imported artifacts to suggest names for.",
          });
          return;
        }
        const response = await api.bulkRenameSuggestions(
          projectSlug,
          rows.map((r) => r.uuid),
        );
        if (cancelled) return;
        setStage({ kind: "results", rows, entries: response.results });
      } catch (err) {
        if (cancelled) return;
        setStage({ kind: "error", error: String(err) });
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
    // We deliberately run exactly once per mount — the arg
    // tuple is captured via the props.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="rename-wizard-heading"
      data-testid="doorstop-rename-wizard"
      className="fixed inset-0 z-20 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="max-h-[90vh] w-full max-w-3xl overflow-auto rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="rename-wizard-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Suggest names for imported artifacts
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          ReqForge asks the configured LLM for alternative filenames. Pick one
          suggestion per row, then apply the selected renames. Artifacts without
          a selection are untouched.
        </p>
        {stage.kind === "loading" ? (
          <p className="mt-6 text-sm text-slate-500" role="status">
            Gathering imported artifacts and asking the LLM…
          </p>
        ) : stage.kind === "error" ? (
          <p className="mt-6 text-sm text-rose-600" role="alert">
            {stage.error}
          </p>
        ) : (
          <ResultsPanel
            projectSlug={projectSlug}
            rows={stage.rows}
            entries={stage.entries}
            onClose={onClose}
          />
        )}
      </div>
    </div>
  );
}

interface ResultsPanelProps {
  readonly projectSlug: string;
  readonly rows: ArtifactRow[];
  readonly entries: BulkRenameSuggestionEntry[];
  readonly onClose: () => void;
}

function ResultsPanel({
  projectSlug,
  rows,
  entries,
  onClose,
}: ResultsPanelProps) {
  const byUuid = useMemo(() => indexEntriesByUuid(entries), [entries]);
  const [picks, setPicks] = useState<Record<string, string>>({});
  const [applied, setApplied] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [isApplying, setIsApplying] = useState(false);

  const pickedCount = Object.keys(picks).length;

  const applyAll = async () => {
    setIsApplying(true);
    const nextApplied: Record<string, string> = {};
    const nextErrors: Record<string, string> = {};
    for (const [uuid, name] of Object.entries(picks)) {
      try {
        await api.renameArtifact(uuid, { name });
        nextApplied[uuid] = name;
      } catch (err) {
        nextErrors[uuid] = String(err);
      }
    }
    setApplied(nextApplied);
    setErrors(nextErrors);
    setIsApplying(false);
  };

  return (
    <div className="mt-4 space-y-3">
      <ul data-testid="doorstop-rename-wizard-list" className="space-y-2">
        {rows.map((row) => {
          const entry = byUuid.get(row.uuid);
          const appliedName = applied[row.uuid];
          const error = errors[row.uuid];
          return (
            <li
              key={row.uuid}
              className="rounded border border-slate-200 p-3 text-xs dark:border-slate-700"
            >
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <div>
                  <span className="font-mono">
                    {row.collectionPrefix} ·{" "}
                    <strong>{appliedName ?? row.currentName}</strong>
                  </span>
                  <span className="ml-2 text-slate-600 dark:text-slate-400">
                    {row.title}
                  </span>
                </div>
                {appliedName ? (
                  <span className="rounded bg-emerald-100 px-1.5 py-0.5 text-[10px] font-medium text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-100">
                    renamed
                  </span>
                ) : null}
              </div>
              <SuggestionCell
                entry={entry}
                picked={picks[row.uuid]}
                onPick={(name) =>
                  setPicks((prev) => ({ ...prev, [row.uuid]: name }))
                }
                onClear={() =>
                  setPicks((prev) => {
                    const next = { ...prev };
                    delete next[row.uuid];
                    return next;
                  })
                }
                disabled={Boolean(appliedName)}
              />
              {error ? (
                <p className="mt-1 text-rose-600" role="alert">
                  {error}
                </p>
              ) : null}
            </li>
          );
        })}
      </ul>
      <div className="flex items-center justify-between gap-2 border-t border-slate-200 pt-3 dark:border-slate-700">
        <p className="text-xs text-slate-600 dark:text-slate-400">
          {pickedCount} of {rows.length} selected · project{" "}
          <span className="font-mono">{projectSlug}</span>
        </p>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Close
          </button>
          <button
            type="button"
            onClick={applyAll}
            disabled={pickedCount === 0 || isApplying}
            data-testid="doorstop-rename-wizard-apply"
            className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
          >
            {isApplying
              ? "Applying…"
              : `Apply ${pickedCount} rename${pickedCount === 1 ? "" : "s"}`}
          </button>
        </div>
      </div>
    </div>
  );
}

interface SuggestionCellProps {
  readonly entry: BulkRenameSuggestionEntry | undefined;
  readonly picked: string | undefined;
  readonly onPick: (name: string) => void;
  readonly onClear: () => void;
  readonly disabled: boolean;
}

function SuggestionCell({
  entry,
  picked,
  onPick,
  onClear,
  disabled,
}: SuggestionCellProps) {
  if (!entry) {
    return (
      <p className="mt-2 text-slate-500">
        No response for this artifact (unexpected).
      </p>
    );
  }
  if (entry.kind === "error") {
    return (
      <p className="mt-2 text-rose-600" role="alert">
        {entry.error}
      </p>
    );
  }
  if (entry.kind === "notFound") {
    return (
      <p className="mt-2 text-slate-500">
        Artifact no longer exists in the project.
      </p>
    );
  }
  if (entry.kind === "privacyAckRequired") {
    return (
      <p className="mt-2 text-amber-700 dark:text-amber-300" role="alert">
        Privacy warning not yet acknowledged for provider
        {entry.indices.length === 1 ? "" : "s"} {entry.indices.join(", ")}.{" "}
        <a
          href="/llm"
          className="underline underline-offset-2 hover:text-amber-900 dark:hover:text-amber-100"
        >
          Acknowledge in LLM providers →
        </a>
      </p>
    );
  }
  return (
    <ul className="mt-2 space-y-1">
      {entry.suggestions.map((s: RenameSuggestion) => {
        const isPicked = picked === s.name;
        return (
          <li key={s.name} className="flex flex-wrap items-baseline gap-2">
            <button
              type="button"
              onClick={() => (isPicked ? onClear() : onPick(s.name))}
              disabled={disabled}
              data-testid={`doorstop-rename-wizard-suggest-${s.name}`}
              className={
                isPicked
                  ? "rounded border border-slate-900 bg-slate-900 px-2 py-0.5 font-mono text-xs text-white dark:border-slate-100 dark:bg-slate-100 dark:text-slate-900"
                  : "rounded border border-slate-300 bg-white px-2 py-0.5 font-mono text-xs hover:bg-slate-100 dark:border-slate-600 dark:bg-slate-900 dark:hover:bg-slate-800"
              }
            >
              {s.name}
            </button>
            <span className="text-slate-600 dark:text-slate-400">
              {s.rationale}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

function indexEntriesByUuid(
  entries: BulkRenameSuggestionEntry[],
): Map<string, BulkRenameSuggestionEntry> {
  const m = new Map<string, BulkRenameSuggestionEntry>();
  for (const e of entries) {
    m.set(e.uuid, e);
  }
  return m;
}
