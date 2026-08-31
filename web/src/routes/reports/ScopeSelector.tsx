import { useId, useMemo } from "react";

import { useCollections, useProjects } from "../../api/queries";
import type { ReportScopeParam } from "../../api/types";

interface Props {
  readonly value: ReportScopeParam;
  readonly onChange: (scope: ReportScopeParam) => void;
}

/// Three-level System / Project / Collection cascader driving
/// a `ReportScopeParam` string. Collection options depend on the
/// currently-selected project; flipping projects resets the
/// collection picker back to "All collections".
export function ScopeSelector({ value, onChange }: Props) {
  const projects = useProjects();
  const parsed = useMemo(() => parseScope(value), [value]);
  const collections = useCollections(parsed.projectSlug);
  const projectId = useId();
  const collectionId = useId();

  const onProjectChange = (slug: string) => {
    if (!slug) onChange("system");
    else onChange(`project:${slug}`);
  };

  const onCollectionChange = (prefix: string) => {
    if (!parsed.projectSlug) return;
    if (!prefix) onChange(`project:${parsed.projectSlug}`);
    else onChange(`collection:${parsed.projectSlug}/${prefix}`);
  };

  return (
    <div className="flex flex-wrap items-center gap-3 text-sm">
      <label className="flex items-center gap-2">
        <span
          className="text-xs uppercase tracking-wide text-slate-500"
          id={`${projectId}-label`}
        >
          Project
        </span>
        <select
          id={projectId}
          aria-labelledby={`${projectId}-label`}
          value={parsed.projectSlug ?? ""}
          onChange={(e) => onProjectChange(e.target.value)}
          className="rounded border border-slate-300 bg-white px-2 py-1 text-xs dark:border-slate-600 dark:bg-slate-800"
        >
          <option value="">All projects (System)</option>
          {(projects.data ?? []).map((p) => (
            <option key={p.slug} value={p.slug}>
              {p.name}
            </option>
          ))}
        </select>
      </label>

      <label
        className={`flex items-center gap-2 ${parsed.projectSlug ? "" : "opacity-50"}`}
      >
        <span
          className="text-xs uppercase tracking-wide text-slate-500"
          id={`${collectionId}-label`}
        >
          Collection
        </span>
        <select
          id={collectionId}
          aria-labelledby={`${collectionId}-label`}
          disabled={!parsed.projectSlug}
          value={parsed.collectionPrefix ?? ""}
          onChange={(e) => onCollectionChange(e.target.value)}
          className="rounded border border-slate-300 bg-white px-2 py-1 text-xs disabled:bg-slate-100 dark:border-slate-600 dark:bg-slate-800 dark:disabled:bg-slate-900"
        >
          <option value="">All collections</option>
          {(collections.data ?? []).map((c) => (
            <option key={c.prefix} value={c.prefix}>
              {c.prefix}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}

function parseScope(scope: ReportScopeParam): {
  projectSlug?: string;
  collectionPrefix?: string;
} {
  // Defensive: callers pass `ReportScopeParam` (always a string) at
  // the type level, but a corrupt saved config blob could round-
  // trip a non-string here. Collapse anything unexpected down to
  // "system" rather than crashing the picker.
  if (typeof scope !== "string") return {};
  if (scope === "system") return {};
  if (scope.startsWith("project:")) {
    return { projectSlug: scope.slice("project:".length) };
  }
  if (scope.startsWith("collection:")) {
    const rest = scope.slice("collection:".length);
    const idx = rest.indexOf("/");
    if (idx < 0) return { projectSlug: rest };
    return {
      projectSlug: rest.slice(0, idx),
      collectionPrefix: rest.slice(idx + 1),
    };
  }
  return {};
}
