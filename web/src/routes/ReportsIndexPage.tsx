import { Link } from "react-router-dom";

import type { ReportKind } from "../api/types";

interface ReportTile {
  kind: ReportKind;
  title: string;
  description: string;
  route: string;
  status: "live" | "upcoming";
}

const TILES: ReportTile[] = [
  {
    kind: "unresolved-links",
    title: "Unresolved links",
    description:
      "Traceability links whose target UUID can't be resolved against the mounted projects.",
    route: "/reports/unresolved-links",
    status: "live",
  },
  {
    kind: "link-orphans",
    title: "Link-graph orphans",
    description:
      "Artifacts with no incoming or outgoing traceability links — typically stale or miscategorised.",
    route: "/reports/link-orphans",
    status: "live",
  },
  {
    kind: "cycles",
    title: "Cycles",
    description:
      "Cycles in link types that are expected to be acyclic (e.g. derives-from, supersedes).",
    route: "/reports/cycles",
    status: "live",
  },
  {
    kind: "conflicts",
    title: "Conflicts",
    description: "Pairs of artifacts related by the conflicts-with link type.",
    route: "/reports/conflicts",
    status: "live",
  },
  {
    kind: "coverage-matrix",
    title: "Coverage matrix",
    description:
      "Parent artifacts vs their covering children; gaps highlighted. Default covering set {satisfies, verifies}.",
    route: "/reports/coverage-matrix",
    status: "live",
  },
  {
    kind: "impact-analysis",
    title: "Impact analysis",
    description:
      "Given a seed artifact, everything transitively dependent on (or depended on by) it.",
    route: "/reports/impact-analysis",
    status: "live",
  },
  {
    kind: "review-status",
    title: "Review status",
    description:
      "Approved / rejected / re-requested / never-reviewed counts by project, collection, and artifact shape.",
    route: "/reports/review-status",
    status: "live",
  },
  {
    kind: "filesystem-orphans",
    title: "Filesystem orphans",
    description:
      "Blob / sidecar pairing mismatches on disk, with an Adopt-as-artifact wizard.",
    route: "/reports/filesystem-orphans",
    status: "live",
  },
  {
    kind: "code-traceability",
    title: "Code traceability",
    description:
      "Per-artifact source / test locations from in-code tags, orphan tags, and uncovered artifacts.",
    route: "/reports/code-traceability",
    status: "live",
  },
];

export function ReportsIndexPage() {
  return (
    <section aria-labelledby="reports-heading" className="space-y-6">
      <header>
        <h1
          id="reports-heading"
          className="text-2xl font-semibold tracking-tight"
        >
          Reports
        </h1>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          Nine report classes over the mounted projects. All take a scope
          selector and an inactive-filter toggle.
        </p>
      </header>

      <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {TILES.map((tile) => (
          <li key={tile.kind}>
            <Link
              to={tile.route}
              className="block rounded-lg border border-slate-200 bg-white p-4 transition hover:border-slate-400 dark:border-slate-700 dark:bg-slate-900 dark:hover:border-slate-500"
            >
              <div className="flex items-baseline justify-between">
                <h2 className="text-base font-semibold">{tile.title}</h2>
                {tile.status === "upcoming" ? (
                  <span className="rounded bg-slate-100 px-1.5 py-0.5 text-xs text-slate-600 dark:bg-slate-800 dark:text-slate-300">
                    upcoming
                  </span>
                ) : null}
              </div>
              <p className="mt-2 text-sm text-slate-600 dark:text-slate-400">
                {tile.description}
              </p>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}
