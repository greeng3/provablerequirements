import { useEffect, useId, useMemo } from "react";

import { useCollections, useProjects } from "../../api/queries";
import type { ReportScopeParam } from "../../api/types";

interface Props {
  readonly value: ReportScopeParam;
  readonly onChange: (scope: ReportScopeParam) => void;
}

/// Single-subject collection narrower. The backend serves exactly
/// one project, so there is no System or Project level to pick —
/// the scope defaults to the whole project and the operator only
/// chooses whether to narrow to a single collection within it.
/// The emitted contract is preserved: `project:{slug}` for the
/// whole project, `collection:{slug}/{prefix}` for one collection.
export function ScopeSelector({ value, onChange }: Props) {
  const projects = useProjects();
  const slug = projects.data?.[0]?.slug;
  const parsed = useMemo(() => parseScope(value), [value]);
  const collections = useCollections(slug);
  const collectionId = useId();

  // Anchor the scope to the single project as soon as it resolves.
  // Anything not already scoped to this project (a stale "system"
  // default, or a scope pointing at a different slug) collapses to
  // the whole project.
  useEffect(() => {
    if (!slug) return;
    if (parsed.projectSlug !== slug) onChange(`project:${slug}`);
  }, [slug, parsed.projectSlug, onChange]);

  const selectedPrefix =
    parsed.projectSlug === slug ? (parsed.collectionPrefix ?? "") : "";

  const onCollectionChange = (prefix: string) => {
    if (!slug) return;
    if (!prefix) onChange(`project:${slug}`);
    else onChange(`collection:${slug}/${prefix}`);
  };

  return (
    <div className="flex flex-wrap items-center gap-3 text-sm">
      <label className="flex items-center gap-2">
        <span
          className="text-xs uppercase tracking-wide text-slate-500"
          id={`${collectionId}-label`}
        >
          Collection
        </span>
        <select
          id={collectionId}
          aria-labelledby={`${collectionId}-label`}
          disabled={!slug}
          value={selectedPrefix}
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
  // the type level, but a stale saved config blob could round-trip
  // a non-string or a legacy "system" scope here. Anything the
  // picker doesn't understand collapses to "no project", and the
  // anchoring effect re-points it at the single project.
  if (typeof scope !== "string") return {};
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
