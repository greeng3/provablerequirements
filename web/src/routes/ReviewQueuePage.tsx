import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useReviewQueue } from "../api/queries";
import type {
  ArtifactShape,
  ReviewQueueEntry,
  ReviewQueueFilters,
} from "../api/types";

/// System-wide review queue per `UX-reviewQueue`. Two sections
/// rendered back-to-back: Awaiting review (default oldest-first,
/// ordering toggleable) and Blocking TODOs (author-side work).
/// Filters are all server-side; this page is just a thin UI over
/// the `/api/reviews/queue` endpoint.
export function ReviewQueuePage() {
  const [filters, setFilters] = useState<ReviewQueueFilters>({
    order: "oldest-first",
  });
  const query = useReviewQueue(filters);

  const awaiting = query.data?.awaitingReview ?? [];
  const blocking = query.data?.blockingTodos ?? [];

  const allProjects = useMemo(
    () => collectValues([...awaiting, ...blocking], "projectSlug"),
    [awaiting, blocking],
  );

  const allCollections = useMemo(
    () => collectValues([...awaiting, ...blocking], "collectionPrefix"),
    [awaiting, blocking],
  );

  const allReviewers = useMemo(
    () =>
      collectValues(
        [...awaiting, ...blocking].filter((e) => e.lastReviewer),
        "lastReviewer",
      ),
    [awaiting, blocking],
  );

  const setFilter = <K extends keyof ReviewQueueFilters>(
    key: K,
    value: ReviewQueueFilters[K] | "",
  ) => {
    setFilters((prev) => ({
      ...prev,
      [key]: value === "" ? undefined : value,
    }));
  };

  return (
    <section className="space-y-6" aria-labelledby="review-queue-heading">
      <header>
        <h1
          id="review-queue-heading"
          className="text-2xl font-semibold tracking-tight"
        >
          Review queue
        </h1>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          Artifacts awaiting a reviewer's attention, plus author work blocked by
          open TODOs.
        </p>
      </header>

      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-5">
        <SelectFilter
          label="Project"
          value={filters.projectSlug ?? ""}
          options={allProjects}
          onChange={(v) => setFilter("projectSlug", v)}
        />
        <SelectFilter
          label="Collection"
          value={filters.collectionPrefix ?? ""}
          options={allCollections}
          onChange={(v) => setFilter("collectionPrefix", v)}
        />
        <SelectFilter
          label="Shape"
          value={filters.shape ?? ""}
          options={["content", "blob", "url"]}
          onChange={(v) =>
            setFilter("shape", (v || undefined) as ArtifactShape | undefined)
          }
        />
        <label className="block text-xs font-medium text-slate-600 dark:text-slate-400">
          Tag
          <input
            type="text"
            value={filters.tag ?? ""}
            onChange={(e) => setFilter("tag", e.target.value)}
            className="mt-1 w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <SelectFilter
          label="Last reviewer"
          value={filters.reviewer ?? ""}
          options={allReviewers}
          onChange={(v) => setFilter("reviewer", v)}
        />
      </div>
      <div className="flex items-center gap-3 text-sm">
        <span className="text-slate-600 dark:text-slate-400">Order:</span>
        <OrderToggle
          value={filters.order ?? "oldest-first"}
          onChange={(v) => setFilter("order", v)}
        />
      </div>

      {query.isError ? (
        <p className="text-sm text-rose-600" role="alert">
          Could not load the review queue.
        </p>
      ) : null}

      <QueueSection
        heading="Awaiting review"
        description="Nothing here means nothing's waiting on a reviewer."
        entries={awaiting}
        empty="No artifacts awaiting review."
      />
      <QueueSection
        heading="Blocking TODOs"
        description="These artifacts are waiting on the author, not on a reviewer."
        entries={blocking}
        empty="No blocking TODOs."
      />
    </section>
  );
}

function QueueSection({
  heading,
  description,
  entries,
  empty,
}: {
  heading: string;
  description: string;
  entries: ReviewQueueEntry[];
  empty: string;
}) {
  return (
    <section className="space-y-2">
      <header>
        <h2 className="text-sm font-semibold tracking-wide text-slate-700 dark:text-slate-300">
          {heading} ({entries.length})
        </h2>
        <p className="text-xs text-slate-500">{description}</p>
      </header>
      {entries.length === 0 ? (
        <p className="text-sm text-slate-500">{empty}</p>
      ) : (
        <ul className="space-y-1 text-sm">
          {entries.map((e) => (
            <li
              key={e.uuid}
              className="flex flex-wrap items-center gap-2 rounded border border-slate-200 p-2 dark:border-slate-700"
            >
              <Link
                to={`/artifacts/${e.uuid}`}
                className="text-slate-800 hover:underline dark:text-slate-200"
              >
                <span className="font-mono text-slate-500">
                  {e.projectSlug}/{e.collectionPrefix}/
                </span>
                {e.artifactName}
                <span className="ml-1 text-slate-500">— {e.title}</span>
              </Link>
              <span className="ml-auto text-xs text-slate-500">
                modified {e.modifiedAt}
              </span>
              {e.blockingTodoCount > 0 ? (
                <span className="rounded-full bg-amber-100 px-2 py-0.5 text-xs text-amber-800 dark:bg-amber-900/50 dark:text-amber-100">
                  {e.blockingTodoCount} TODO
                  {e.blockingTodoCount === 1 ? "" : "s"}
                </span>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function SelectFilter({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: string[];
  onChange: (value: string) => void;
}) {
  return (
    <label className="block text-xs font-medium text-slate-600 dark:text-slate-400">
      {label}
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="mt-1 w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
      >
        <option value="">Any</option>
        {options.map((opt) => (
          <option key={opt} value={opt}>
            {opt}
          </option>
        ))}
      </select>
    </label>
  );
}

function OrderToggle({
  value,
  onChange,
}: {
  value: "oldest-first" | "newest-first";
  onChange: (v: "oldest-first" | "newest-first") => void;
}) {
  return (
    <div className="inline-flex overflow-hidden rounded border border-slate-300 dark:border-slate-600">
      <button
        type="button"
        onClick={() => onChange("oldest-first")}
        className={
          value === "oldest-first"
            ? "bg-slate-900 px-2 py-0.5 text-xs text-white dark:bg-slate-100 dark:text-slate-900"
            : "px-2 py-0.5 text-xs"
        }
      >
        Oldest first
      </button>
      <button
        type="button"
        onClick={() => onChange("newest-first")}
        className={
          value === "newest-first"
            ? "bg-slate-900 px-2 py-0.5 text-xs text-white dark:bg-slate-100 dark:text-slate-900"
            : "px-2 py-0.5 text-xs"
        }
      >
        Newest first
      </button>
    </div>
  );
}

function collectValues<K extends keyof ReviewQueueEntry>(
  entries: ReviewQueueEntry[],
  key: K,
): string[] {
  const set = new Set<string>();
  for (const e of entries) {
    const v = e[key];
    if (typeof v === "string" && v.length > 0) {
      set.add(v);
    }
  }
  return Array.from(set).sort();
}
