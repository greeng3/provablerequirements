import type {
  MatrixNodeDto,
  MatrixReviewStateTag,
  ReportScopeParam,
} from "../../../api/types";
import { MATRIX_REVIEW_STATE_TAGS } from "../../../api/types";
import { ScopeSelector } from "../../reports/ScopeSelector";

interface Props {
  readonly title: string;
  readonly scope: ReportScopeParam;
  readonly onScopeChange: (scope: ReportScopeParam) => void;
  readonly tags: string[];
  readonly onTagsChange: (next: string[]) => void;
  readonly reviewStates: MatrixReviewStateTag[];
  readonly onReviewStatesChange: (next: MatrixReviewStateTag[]) => void;
  /// Nodes currently on this axis; the tag chip universe is
  /// union-derived from the live response (same cheap trick as
  /// the 7a GraphFilters) so operators see only tags that
  /// actually apply.
  readonly nodes: MatrixNodeDto[];
}

/// Filter block for one axis (Rows or Columns). Mirror-image of
/// the other axis's filters so operators can flip a matrix's
/// orientation without relearning the controls.
export function MatrixAxisFilters({
  title,
  scope,
  onScopeChange,
  tags,
  onTagsChange,
  reviewStates,
  onReviewStatesChange,
  nodes,
}: Props) {
  const tagUniverse = collectTagUniverse(nodes);
  const toggleTag = (value: string) =>
    onTagsChange(
      tags.includes(value) ? tags.filter((t) => t !== value) : [...tags, value],
    );
  const toggleReview = (value: MatrixReviewStateTag) =>
    onReviewStatesChange(
      reviewStates.includes(value)
        ? reviewStates.filter((t) => t !== value)
        : [...reviewStates, value],
    );

  return (
    <section
      aria-label={`${title} filters`}
      className="space-y-2 rounded border border-slate-200 p-3 dark:border-slate-800"
    >
      <h2 className="text-xs font-semibold uppercase tracking-wide text-slate-500">
        {title}
      </h2>
      <ScopeSelector value={scope} onChange={onScopeChange} />

      <ChipGroup
        label="Review state"
        emptyMessage="(no filter)"
        items={MATRIX_REVIEW_STATE_TAGS.map((t) => ({
          value: t,
          label: t,
        }))}
        selected={reviewStates}
        onToggle={(v) => toggleReview(v as MatrixReviewStateTag)}
        onClear={() => onReviewStatesChange([])}
      />
      <ChipGroup
        label="Tags"
        emptyMessage="No tags on visible nodes."
        items={tagUniverse.map((t) => ({ value: t, label: t }))}
        selected={tags}
        onToggle={toggleTag}
        onClear={() => onTagsChange([])}
      />
    </section>
  );
}

interface ChipGroupProps<T extends string> {
  readonly label: string;
  readonly emptyMessage: string;
  readonly items: { value: T; label: string }[];
  readonly selected: T[];
  readonly onToggle: (value: T) => void;
  readonly onClear: () => void;
}

function ChipGroup<T extends string>({
  label,
  emptyMessage,
  items,
  selected,
  onToggle,
  onClear,
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
        <p className="text-xs text-slate-500">{emptyMessage}</p>
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

function collectTagUniverse(nodes: MatrixNodeDto[]): string[] {
  const seen = new Set<string>();
  for (const n of nodes) {
    for (const t of n.tags) seen.add(t);
  }
  return Array.from(seen).sort();
}
