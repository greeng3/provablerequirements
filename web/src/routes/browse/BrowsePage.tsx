import { useMemo, useState } from "react";

import { useBrowse } from "../../api/queries";
import type {
  BrowsePane as BrowsePaneDto,
  MatrixReviewStateTag,
  ReportScopeParam,
} from "../../api/types";
import { MATRIX_REVIEW_STATE_TAGS } from "../../api/types";
import { ScopeSelector } from "../reports/ScopeSelector";

import { BrowsePane } from "./BrowsePane";

/// Top-level browse-by-type page. Each pane groups artifacts
/// whose Collections share a prefix across every mounted
/// project; the filter row narrows each axis globally.
export function BrowsePage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [tags, setTags] = useState<string[]>([]);
  const [reviewStates, setReviewStates] = useState<MatrixReviewStateTag[]>([]);
  const [includeInactive, setIncludeInactive] = useState(false);

  const params = useMemo(
    () => ({
      scope,
      tags: tags.length > 0 ? tags : undefined,
      reviewState: reviewStates.length > 0 ? reviewStates : undefined,
      includeInactive,
    }),
    [scope, tags, reviewStates, includeInactive],
  );
  const query = useBrowse(params);
  const data = query.data;

  const tagUniverse = useMemo(() => collectTagUniverse(data?.panes), [data]);

  const toggleTag = (value: string) =>
    setTags((prev) =>
      prev.includes(value) ? prev.filter((t) => t !== value) : [...prev, value],
    );
  const toggleReview = (value: MatrixReviewStateTag) =>
    setReviewStates((prev) =>
      prev.includes(value) ? prev.filter((t) => t !== value) : [...prev, value],
    );

  return (
    <section className="space-y-4">
      <header className="space-y-1 border-b border-slate-200 pb-3 dark:border-slate-800">
        <h1 className="text-2xl font-semibold tracking-tight">Browse</h1>
        <p className="text-sm text-slate-600 dark:text-slate-400">
          Per-type indexes: one pane per distinct Collection prefix across every
          mounted project. Artifacts within a pane are sorted by title. Use the
          filters to narrow, then expand a pane to scan.
        </p>
      </header>

      <div className="space-y-3 border-b border-slate-200 pb-3 dark:border-slate-800">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <ScopeSelector value={scope} onChange={setScope} />
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={includeInactive}
              onChange={(e) => setIncludeInactive(e.target.checked)}
            />
            <span>Include inactive</span>
          </label>
        </div>
        <div className="flex flex-wrap items-start gap-6">
          <ChipGroup
            label="Review state"
            items={MATRIX_REVIEW_STATE_TAGS.map((r) => ({
              value: r,
              label: r,
            }))}
            selected={reviewStates}
            onToggle={(v) => toggleReview(v as MatrixReviewStateTag)}
            onClear={() => setReviewStates([])}
          />
          <ChipGroup
            label="Tags"
            items={tagUniverse.map((t) => ({ value: t, label: t }))}
            selected={tags}
            onToggle={toggleTag}
            onClear={() => setTags([])}
            emptyMessage="No tags on visible artifacts."
          />
        </div>
      </div>

      {query.isLoading ? (
        <p className="text-sm text-slate-500">Loading…</p>
      ) : query.isError || !data ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load browse view: {String(query.error ?? "unknown")}
        </p>
      ) : (
        <BrowseBody panes={data.panes} totalArtifacts={data.totalArtifacts} />
      )}
    </section>
  );
}

interface BodyProps {
  readonly panes: BrowsePaneDto[];
  readonly totalArtifacts: number;
}

function BrowseBody({ panes, totalArtifacts }: BodyProps) {
  if (panes.length === 0) {
    return (
      <p className="text-sm text-slate-500">
        No artifacts match the current filters. Try broadening the scope or
        clearing the filters.
      </p>
    );
  }
  return (
    <div className="space-y-3">
      <p data-testid="browse-totals" className="text-xs text-slate-500">
        {panes.length} pane{panes.length === 1 ? "" : "s"} · {totalArtifacts}{" "}
        artifact{totalArtifacts === 1 ? "" : "s"}
      </p>
      <div className="space-y-2">
        {panes.map((pane) => (
          <BrowsePane key={pane.prefix} pane={pane} />
        ))}
      </div>
    </div>
  );
}

interface ChipGroupProps<T extends string> {
  readonly label: string;
  readonly items: { value: T; label: string }[];
  readonly selected: T[];
  readonly onToggle: (value: T) => void;
  readonly onClear: () => void;
  readonly emptyMessage?: string;
}

function ChipGroup<T extends string>({
  label,
  items,
  selected,
  onToggle,
  onClear,
  emptyMessage,
}: ChipGroupProps<T>) {
  return (
    <div className="space-y-1">
      <div className="flex items-center gap-2">
        <p className="text-xs uppercase tracking-wide text-slate-500">
          {label}
        </p>
        {selected.length > 0 ? (
          <button
            type="button"
            onClick={onClear}
            className="text-xs text-slate-500 underline hover:text-slate-700 dark:hover:text-slate-300"
          >
            clear
          </button>
        ) : null}
      </div>
      {items.length === 0 ? (
        emptyMessage ? (
          <p className="text-xs text-slate-500">{emptyMessage}</p>
        ) : null
      ) : (
        <div className="flex flex-wrap gap-1">
          {items.map((item) => {
            const active = selected.includes(item.value);
            return (
              <button
                key={item.value}
                type="button"
                onClick={() => onToggle(item.value)}
                aria-pressed={active}
                className={`rounded border px-2 py-0.5 font-mono text-xs ${
                  active
                    ? "border-sky-500 bg-sky-50 text-sky-800 dark:border-sky-400 dark:bg-sky-900/30 dark:text-sky-100"
                    : "border-slate-300 text-slate-700 hover:bg-slate-100 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
                }`}
              >
                {item.label}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function collectTagUniverse(panes: BrowsePaneDto[] | undefined): string[] {
  if (!panes) return [];
  const seen = new Set<string>();
  for (const pane of panes) {
    for (const a of pane.artifacts) {
      for (const t of a.tags) seen.add(t);
    }
  }
  return Array.from(seen).sort();
}
