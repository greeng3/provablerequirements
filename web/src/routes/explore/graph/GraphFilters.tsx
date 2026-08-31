import type { GraphNodeDto, ReportScopeParam } from "../../../api/types";
import { ScopeSelector } from "../../reports/ScopeSelector";

import type { LayoutKind } from "./layouts/types";

interface Props {
  readonly scope: ReportScopeParam;
  readonly onScopeChange: (scope: ReportScopeParam) => void;
  readonly includeInactive: boolean;
  readonly onIncludeInactiveChange: (value: boolean) => void;
  readonly selectedLinkTypes: string[];
  readonly onLinkTypesChange: (next: string[]) => void;
  readonly selectedTags: string[];
  readonly onTagsChange: (next: string[]) => void;
  readonly layout: LayoutKind;
  readonly onLayoutChange: (layout: LayoutKind) => void;
  readonly linkTypeNames: string[];
  readonly nodes: GraphNodeDto[];
}

/// Filter row above the canvas. Tag choices are union-derived
/// from the current node set rather than a separate endpoint —
/// cheap because the truncated response is bounded at 500 nodes.
export function GraphFilters({
  scope,
  onScopeChange,
  includeInactive,
  onIncludeInactiveChange,
  selectedLinkTypes,
  onLinkTypesChange,
  selectedTags,
  onTagsChange,
  layout,
  onLayoutChange,
  linkTypeNames,
  nodes,
}: Props) {
  const tagUniverse = collectTagUniverse(nodes);

  const toggle = (current: string[], value: string): string[] =>
    current.includes(value)
      ? current.filter((v) => v !== value)
      : [...current, value];

  return (
    <div className="space-y-3 border-b border-slate-200 pb-3 dark:border-slate-800">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <ScopeSelector value={scope} onChange={onScopeChange} />
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={includeInactive}
              onChange={(e) => onIncludeInactiveChange(e.target.checked)}
            />
            <span>Include inactive</span>
          </label>
          <fieldset className="flex items-center gap-1 text-xs">
            <legend className="sr-only">Layout engine</legend>
            <LayoutButton
              active={layout === "hierarchical"}
              onClick={() => onLayoutChange("hierarchical")}
            >
              Hierarchical
            </LayoutButton>
            <LayoutButton
              active={layout === "force"}
              onClick={() => onLayoutChange("force")}
            >
              Force
            </LayoutButton>
          </fieldset>
        </div>
      </div>

      <div className="flex flex-wrap items-start gap-6">
        <FilterChipGroup
          label="Link types"
          emptyMessage="No link types loaded yet."
          items={linkTypeNames.map((name) => ({ value: name, label: name }))}
          selected={selectedLinkTypes}
          onToggle={(name) =>
            onLinkTypesChange(toggle(selectedLinkTypes, name))
          }
          onClear={() => onLinkTypesChange([])}
        />
        <FilterChipGroup
          label="Tags"
          emptyMessage="No tags on visible nodes."
          items={tagUniverse.map((t) => ({ value: t, label: t }))}
          selected={selectedTags}
          onToggle={(tag) => onTagsChange(toggle(selectedTags, tag))}
          onClear={() => onTagsChange([])}
        />
      </div>
    </div>
  );
}

interface LayoutButtonProps {
  readonly active: boolean;
  readonly onClick: () => void;
  readonly children: React.ReactNode;
}

function LayoutButton({ active, onClick, children }: LayoutButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={`rounded border px-2 py-1 ${
        active
          ? "border-sky-500 bg-sky-50 text-sky-800 dark:border-sky-400 dark:bg-sky-900/30 dark:text-sky-100"
          : "border-slate-300 hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
      }`}
    >
      {children}
    </button>
  );
}

interface FilterChipGroupProps {
  readonly label: string;
  readonly emptyMessage: string;
  readonly items: { value: string; label: string }[];
  readonly selected: string[];
  readonly onToggle: (value: string) => void;
  readonly onClear: () => void;
}

function FilterChipGroup({
  label,
  emptyMessage,
  items,
  selected,
  onToggle,
  onClear,
}: FilterChipGroupProps) {
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

function collectTagUniverse(nodes: GraphNodeDto[]): string[] {
  const seen = new Set<string>();
  for (const node of nodes) {
    for (const tag of node.tags) seen.add(tag);
  }
  return Array.from(seen).sort();
}
