import { Link } from "react-router-dom";

import type { CollectionSummary } from "../../api/types";

export function CollectionCard({
  projectSlug,
  collection,
}: {
  projectSlug: string;
  collection: CollectionSummary;
}) {
  return (
    <Link
      to={`/projects/${projectSlug}/collections/${collection.prefix}`}
      className="flex items-start justify-between gap-4 rounded-lg border border-slate-200 bg-white p-4 shadow-sm transition hover:border-slate-300 hover:bg-slate-50 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-slate-700 dark:hover:bg-slate-800"
    >
      <div className="min-w-0">
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-sm font-semibold text-slate-700 dark:text-slate-200">
            {collection.prefix}
          </span>
          <span className="truncate text-base font-medium text-slate-900 dark:text-slate-100">
            {collection.name}
          </span>
        </div>
        {collection.description ? (
          <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
            {collection.description}
          </p>
        ) : null}
      </div>
      <div className="shrink-0 text-right text-sm text-slate-500">
        <span className="font-semibold text-slate-900 dark:text-slate-100">
          {collection.artifactCount}
        </span>{" "}
        {collection.artifactCount === 1 ? "artifact" : "artifacts"}
      </div>
    </Link>
  );
}
