import type {
  MatrixReviewStateTag,
  ReportScopeParam,
  SearchHasLinksFilter,
  SearchShapeTag,
} from "../../api/types";
import { MATRIX_REVIEW_STATE_TAGS } from "../../api/types";
import { ScopeSelector } from "../reports/ScopeSelector";

interface Props {
  readonly scope: ReportScopeParam;
  readonly onScopeChange: (scope: ReportScopeParam) => void;
  readonly shapes: SearchShapeTag[];
  readonly onShapesChange: (next: SearchShapeTag[]) => void;
  readonly reviewStates: MatrixReviewStateTag[];
  readonly onReviewStatesChange: (next: MatrixReviewStateTag[]) => void;
  readonly hasLinks: SearchHasLinksFilter;
  readonly onHasLinksChange: (next: SearchHasLinksFilter) => void;
  readonly includeInactive: boolean;
  readonly onIncludeInactiveChange: (value: boolean) => void;
}

const SHAPE_OPTIONS: SearchShapeTag[] = ["content", "blob", "url"];

/// Filter row for the search page. Mirror-image of the Phase
/// 7b matrix axis filters in layout, but with a single set
/// since search has one result axis.
export function SearchFilters({
  scope,
  onScopeChange,
  shapes,
  onShapesChange,
  reviewStates,
  onReviewStatesChange,
  hasLinks,
  onHasLinksChange,
  includeInactive,
  onIncludeInactiveChange,
}: Props) {
  const toggleShape = (value: SearchShapeTag) =>
    onShapesChange(
      shapes.includes(value)
        ? shapes.filter((s) => s !== value)
        : [...shapes, value],
    );
  const toggleReview = (value: MatrixReviewStateTag) =>
    onReviewStatesChange(
      reviewStates.includes(value)
        ? reviewStates.filter((r) => r !== value)
        : [...reviewStates, value],
    );

  return (
    <div className="space-y-3 border-b border-slate-200 pb-3 dark:border-slate-800">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <ScopeSelector value={scope} onChange={onScopeChange} />
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={includeInactive}
            onChange={(e) => onIncludeInactiveChange(e.target.checked)}
          />
          <span>Include inactive</span>
        </label>
      </div>
      <div className="flex flex-wrap items-start gap-6">
        <ChipGroup
          label="Shape"
          items={SHAPE_OPTIONS.map((s) => ({ value: s, label: s }))}
          selected={shapes}
          onToggle={toggleShape}
          onClear={() => onShapesChange([])}
        />
        <ChipGroup
          label="Review state"
          items={MATRIX_REVIEW_STATE_TAGS.map((r) => ({
            value: r,
            label: r,
          }))}
          selected={reviewStates}
          onToggle={toggleReview}
          onClear={() => onReviewStatesChange([])}
        />
        <HasLinksToggle value={hasLinks} onChange={onHasLinksChange} />
      </div>
    </div>
  );
}

interface HasLinksProps {
  readonly value: SearchHasLinksFilter;
  readonly onChange: (next: SearchHasLinksFilter) => void;
}

function HasLinksToggle({ value, onChange }: HasLinksProps) {
  const options: { value: SearchHasLinksFilter; label: string }[] = [
    { value: "any", label: "Any" },
    { value: "true", label: "Has links" },
    { value: "false", label: "No links" },
  ];
  return (
    <fieldset className="space-y-1">
      <legend className="text-xs uppercase tracking-wide text-slate-500">
        Link presence
      </legend>
      <div className="flex flex-wrap gap-1">
        {options.map((o) => (
          <button
            key={o.value}
            type="button"
            aria-pressed={value === o.value}
            onClick={() => onChange(o.value)}
            className={`rounded border px-2 py-0.5 text-xs ${
              value === o.value
                ? "border-sky-500 bg-sky-50 text-sky-800 dark:border-sky-400 dark:bg-sky-900/30 dark:text-sky-100"
                : "border-slate-300 text-slate-700 hover:bg-slate-100 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
            }`}
          >
            {o.label}
          </button>
        ))}
      </div>
    </fieldset>
  );
}

interface ChipGroupProps<T extends string> {
  readonly label: string;
  readonly items: { value: T; label: string }[];
  readonly selected: T[];
  readonly onToggle: (value: T) => void;
  readonly onClear: () => void;
}

function ChipGroup<T extends string>({
  label,
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
    </div>
  );
}
