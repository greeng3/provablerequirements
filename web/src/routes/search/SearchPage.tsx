import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useSearch } from "../../api/queries";
import type {
  MatrixReviewStateTag,
  ReportScopeParam,
  SearchHasLinksFilter,
  SearchHit,
  SearchShapeTag,
} from "../../api/types";
import { SEARCH_DEFAULT_LIMIT } from "../../api/types";
import { useDebounce } from "../../hooks/useDebounce";

import { SearchFilters } from "./SearchFilters";
import { SearchSnippet } from "./SearchSnippet";

/// Top-level search page. Query box debounces input (300 ms)
/// so each keystroke doesn't fire a request; pure-filter
/// searches run through the same empty-query path the backend
/// match-all supports.
export function SearchPage() {
  const [raw, setRaw] = useState("");
  const q = useDebounce(raw, 300);
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [shapes, setShapes] = useState<SearchShapeTag[]>([]);
  const [reviewStates, setReviewStates] = useState<MatrixReviewStateTag[]>([]);
  const [hasLinks, setHasLinks] = useState<SearchHasLinksFilter>("any");
  const [includeInactive, setIncludeInactive] = useState(false);
  const [offset, setOffset] = useState(0);

  const params = useMemo(
    () => ({
      q,
      scope,
      shape: shapes.length > 0 ? shapes : undefined,
      reviewState: reviewStates.length > 0 ? reviewStates : undefined,
      hasLinks: hasLinks === "any" ? undefined : hasLinks === "true",
      includeInactive,
      limit: SEARCH_DEFAULT_LIMIT,
      offset,
    }),
    [q, scope, shapes, reviewStates, hasLinks, includeInactive, offset],
  );

  const query = useSearch(params);
  const data = query.data;

  const resetOffsetAnd = <T,>(setter: (v: T) => void) => {
    return (v: T) => {
      setOffset(0);
      setter(v);
    };
  };

  return (
    <section className="space-y-4">
      <header className="space-y-1 border-b border-slate-200 pb-3 dark:border-slate-800">
        <h1 className="text-2xl font-semibold tracking-tight">Search</h1>
        <p className="text-sm text-slate-600 dark:text-slate-400">
          Full-text search across artifact title, short name, body, description,
          and tags. Tantivy query syntax (phrases, field- scoped queries,
          boolean operators) is supported; combine with the structured filters
          to narrow.
        </p>
      </header>

      <input
        type="search"
        value={raw}
        onChange={(e) => {
          setOffset(0);
          setRaw(e.target.value);
        }}
        placeholder="Search artifacts…"
        aria-label="Search query"
        className="w-full rounded border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-600 dark:bg-slate-800"
      />

      <SearchFilters
        scope={scope}
        onScopeChange={resetOffsetAnd(setScope)}
        shapes={shapes}
        onShapesChange={resetOffsetAnd(setShapes)}
        reviewStates={reviewStates}
        onReviewStatesChange={resetOffsetAnd(setReviewStates)}
        hasLinks={hasLinks}
        onHasLinksChange={resetOffsetAnd(setHasLinks)}
        includeInactive={includeInactive}
        onIncludeInactiveChange={resetOffsetAnd(setIncludeInactive)}
      />

      {query.isLoading ? (
        <p className="text-sm text-slate-500">Searching…</p>
      ) : query.isError || !data ? (
        <p className="text-sm text-rose-600" role="alert">
          Search failed: {String(query.error ?? "unknown")}
        </p>
      ) : (
        <SearchBody
          data={data}
          offset={offset}
          onNext={() => setOffset(offset + SEARCH_DEFAULT_LIMIT)}
          onPrev={() => setOffset(Math.max(0, offset - SEARCH_DEFAULT_LIMIT))}
        />
      )}
    </section>
  );
}

interface BodyProps {
  readonly data: {
    totalHits: number;
    truncated: boolean;
    hits: SearchHit[];
  };
  readonly offset: number;
  readonly onNext: () => void;
  readonly onPrev: () => void;
}

function SearchBody({ data, offset, onNext, onPrev }: BodyProps) {
  if (data.totalHits === 0) {
    return (
      <p className="text-sm text-slate-500">
        No artifacts match the current query and filters.
      </p>
    );
  }
  return (
    <div className="space-y-3">
      <p data-testid="search-totals" className="text-xs text-slate-500">
        {data.totalHits} hit{data.totalHits === 1 ? "" : "s"} · showing{" "}
        {offset + 1}–{offset + data.hits.length}
      </p>
      <ul className="space-y-2">
        {data.hits.map((hit) => (
          <SearchResultRow key={hit.uuid} hit={hit} />
        ))}
      </ul>
      <div className="flex items-center justify-end gap-2">
        <button
          type="button"
          onClick={onPrev}
          disabled={offset === 0}
          className="rounded border border-slate-300 px-3 py-1 text-xs disabled:opacity-50 dark:border-slate-600"
          data-testid="search-prev"
        >
          Previous
        </button>
        <button
          type="button"
          onClick={onNext}
          disabled={!data.truncated}
          className="rounded border border-slate-300 px-3 py-1 text-xs disabled:opacity-50 dark:border-slate-600"
          data-testid="search-next"
        >
          Next
        </button>
      </div>
    </div>
  );
}

function SearchResultRow({ hit }: { hit: SearchHit }) {
  return (
    <li
      data-testid={`search-result-${hit.artifactName}`}
      className="rounded border border-slate-200 p-3 dark:border-slate-800"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <Link
          to={`/projects/${hit.projectSlug}/collections/${hit.collectionPrefix}/artifacts/${hit.artifactName}`}
          className="font-mono text-xs text-sky-700 underline dark:text-sky-300"
        >
          {hit.projectSlug}/{hit.collectionPrefix}/{hit.artifactName}
        </Link>
        <div className="flex items-center gap-2 text-xs">
          <span className="rounded border border-slate-300 px-1.5 py-0.5 dark:border-slate-600">
            {hit.shape}
          </span>
          <span
            className={`rounded px-1.5 py-0.5 ${reviewStatePalette(hit.reviewState)}`}
          >
            {hit.reviewState}
          </span>
          {!hit.active ? (
            <span className="rounded bg-slate-200 px-1.5 py-0.5 text-slate-700 dark:bg-slate-700 dark:text-slate-200">
              inactive
            </span>
          ) : null}
        </div>
      </div>
      <p className="mt-1 text-sm text-slate-800 dark:text-slate-100">
        {hit.title}
      </p>
      {hit.snippet ? <SearchSnippet snippet={hit.snippet} /> : null}
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
