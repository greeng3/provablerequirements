import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import type {
  BrowsePane as BrowsePaneDto,
  MatrixReviewStateTag,
} from "../../api/types";

interface Props {
  readonly pane: BrowsePaneDto;
}

/// One collapsible card per Collection prefix. Operators land
/// with every pane collapsed so the page scans — expanding a
/// pane fetches nothing extra (the artifacts are already in
/// the response).
export function BrowsePane({ pane }: Props) {
  const [open, setOpen] = useState(false);
  const [titleFilter, setTitleFilter] = useState("");

  const visibleArtifacts = useMemo(() => {
    const needle = titleFilter.trim().toLowerCase();
    if (!needle) return pane.artifacts;
    return pane.artifacts.filter(
      (a) =>
        a.title.toLowerCase().includes(needle) ||
        a.artifactName.toLowerCase().includes(needle),
    );
  }, [pane.artifacts, titleFilter]);

  return (
    <section
      data-testid={`browse-pane-${pane.prefix}`}
      className="rounded border border-slate-200 dark:border-slate-800"
    >
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between gap-3 rounded-t px-3 py-2 text-left text-sm hover:bg-slate-50 dark:hover:bg-slate-800"
      >
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-sm font-semibold">{pane.prefix}</span>
          <span className="text-slate-600 dark:text-slate-400">
            {pane.name}
          </span>
          {pane.nameVariants && pane.nameVariants.length > 0 ? (
            <span
              data-testid={`browse-pane-${pane.prefix}-variants`}
              title={`Also seen as: ${pane.nameVariants.join(", ")}`}
              className="rounded bg-amber-100 px-1.5 py-0.5 text-xs text-amber-900 dark:bg-amber-900/40 dark:text-amber-100"
            >
              name drift
            </span>
          ) : null}
        </div>
        <span className="text-xs text-slate-500">
          {pane.totalArtifacts} artifact{pane.totalArtifacts === 1 ? "" : "s"}
          {" · "}
          {open ? "▾" : "▸"}
        </span>
      </button>
      {open ? (
        <div className="space-y-2 border-t border-slate-200 p-3 dark:border-slate-800">
          {pane.totalArtifacts > 0 ? (
            <>
              <input
                type="search"
                value={titleFilter}
                onChange={(e) => setTitleFilter(e.target.value)}
                placeholder="Filter in this pane…"
                aria-label={`Filter ${pane.prefix} artifacts`}
                className="w-full rounded border border-slate-300 bg-white px-2 py-1 text-xs dark:border-slate-600 dark:bg-slate-800"
              />
              <ul className="space-y-1">
                {visibleArtifacts.map((a) => (
                  <BrowseRow key={a.uuid} artifact={a} />
                ))}
              </ul>
              {visibleArtifacts.length === 0 ? (
                <p className="text-xs text-slate-500">
                  No artifacts in this pane match {`"${titleFilter}"`}.
                </p>
              ) : null}
            </>
          ) : (
            <p className="text-xs text-slate-500">
              No artifacts in this collection match the current filters.
            </p>
          )}
        </div>
      ) : null}
    </section>
  );
}

interface RowProps {
  readonly artifact: BrowsePaneDto["artifacts"][number];
}

function BrowseRow({ artifact }: RowProps) {
  return (
    <li
      data-testid={`browse-row-${artifact.artifactName}`}
      className="flex flex-wrap items-baseline justify-between gap-2 rounded border border-slate-200 p-2 text-xs dark:border-slate-800"
    >
      <div className="flex flex-wrap items-baseline gap-2">
        <Link
          to={`/projects/${artifact.projectSlug}/collections/${artifact.collectionPrefix}/artifacts/${artifact.artifactName}`}
          className="font-mono text-sky-700 underline dark:text-sky-300"
        >
          {artifact.projectSlug}/{artifact.collectionPrefix}/
          {artifact.artifactName}
        </Link>
        <span className="text-slate-700 dark:text-slate-200">
          {artifact.title}
        </span>
      </div>
      <div className="flex items-center gap-1">
        <span className="rounded border border-slate-300 px-1.5 py-0.5 dark:border-slate-600">
          {artifact.shape}
        </span>
        <span
          className={`rounded px-1.5 py-0.5 ${reviewStatePalette(artifact.reviewState)}`}
        >
          {artifact.reviewState}
        </span>
        {!artifact.active ? (
          <span className="rounded bg-slate-200 px-1.5 py-0.5 text-slate-700 dark:bg-slate-700 dark:text-slate-200">
            inactive
          </span>
        ) : null}
      </div>
    </li>
  );
}

function reviewStatePalette(state: MatrixReviewStateTag): string {
  switch (state) {
    case "approved":
      return "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-100";
    case "rejected":
      return "bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-100";
    case "re-requested":
      return "bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-100";
    case "never-reviewed":
      return "bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-200";
  }
}
