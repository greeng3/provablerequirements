import { useEffect, useId, useMemo, useState } from "react";

import { useArtifactSearch, useLinkTypes } from "../../api/queries";
import { useDebounce } from "../../hooks/useDebounce";
import type {
  ArtifactSearchResult,
  LinkType,
  LinkWriteRequest,
} from "../../api/types";

/// Two-step picker per UX-linkCreationPicker: the user selects a
/// link type (dropdown over the effective catalog), then a target
/// artifact by typing into a type-ahead that searches every
/// mounted project via GET /api/artifacts/search. Results in the
/// current project bubble to the top so authoring stays
/// project-local by default without hiding cross-project matches.
export function LinkPicker({
  currentArtifactUuid,
  currentProjectSlug,
  onCommit,
  onClose,
}: {
  currentArtifactUuid: string;
  currentProjectSlug: string;
  onCommit: (req: LinkWriteRequest) => void;
  onClose: () => void;
}) {
  const typeId = useId();
  const targetId = useId();
  const typesQuery = useLinkTypes();
  const [typeName, setTypeName] = useState<string>("derives-from");
  const [query, setQuery] = useState<string>("");
  const [highlightIndex, setHighlightIndex] = useState(0);
  const debouncedQuery = useDebounce(query, 150);
  const searchQuery = useArtifactSearch(debouncedQuery, currentArtifactUuid);

  const ordered = useMemo(
    () => rankResults(searchQuery.data ?? [], currentProjectSlug),
    [searchQuery.data, currentProjectSlug],
  );

  useEffect(() => {
    setHighlightIndex(0);
  }, [debouncedQuery]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const commit = (target: ArtifactSearchResult) => {
    onCommit({
      targetUuid: target.uuid,
      type: typeName,
      hint: {
        projectSlug: target.projectSlug,
        collectionPrefix: target.collectionPrefix,
        artifactName: target.artifactName,
      },
    });
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (ordered.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setHighlightIndex((i) => (i + 1) % ordered.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setHighlightIndex((i) => (i - 1 + ordered.length) % ordered.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      commit(ordered[highlightIndex]);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="link-picker-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="link-picker-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Add link
        </h2>

        <div className="mt-4 space-y-3">
          <label className="block text-sm" htmlFor={typeId}>
            Link type
            <select
              id={typeId}
              value={typeName}
              onChange={(e) => setTypeName(e.target.value)}
              className="mt-1 w-full rounded border border-slate-300 bg-white px-2 py-1 dark:border-slate-700 dark:bg-slate-800"
            >
              {groupedTypes(typesQuery.data ?? []).map(([group, types]) => (
                <optgroup key={group} label={group}>
                  {types.map((t) => (
                    <option key={t.name} value={t.name}>
                      {t.name}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>

          <label className="block text-sm" htmlFor={targetId}>
            Target artifact
            <input
              id={targetId}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type to search by name or title"
              className="mt-1 w-full rounded border border-slate-300 bg-white px-2 py-1 dark:border-slate-700 dark:bg-slate-800"
              autoFocus
            />
          </label>

          <ul
            aria-label="Target candidates"
            className="max-h-60 overflow-y-auto rounded border border-slate-200 dark:border-slate-700"
          >
            {debouncedQuery.length === 0 && (
              <li className="px-3 py-2 text-sm text-slate-500">
                Start typing to search.
              </li>
            )}
            {debouncedQuery.length > 0 &&
              ordered.length === 0 &&
              !searchQuery.isFetching && (
                <li className="px-3 py-2 text-sm text-slate-500">
                  No matches.
                </li>
              )}
            {ordered.map((r, index) => (
              <li key={r.uuid}>
                <button
                  type="button"
                  onClick={() => commit(r)}
                  onMouseEnter={() => setHighlightIndex(index)}
                  className={
                    index === highlightIndex
                      ? "w-full px-3 py-2 text-left text-sm bg-slate-100 dark:bg-slate-800"
                      : "w-full px-3 py-2 text-left text-sm hover:bg-slate-50 dark:hover:bg-slate-800"
                  }
                >
                  <span className="font-mono text-slate-500">
                    {r.projectSlug}/{r.collectionPrefix}/
                  </span>
                  {r.artifactName}
                  <span className="ml-2 text-slate-600 dark:text-slate-400">
                    — {r.title}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-slate-300 px-3 py-1 text-sm dark:border-slate-700"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

/// Current project first, then alphabetical by slug, then by
/// artifactName. The server's endpoint already applies the prefix
/// boost; we only layer current-project priority on top.
function rankResults(
  results: ArtifactSearchResult[],
  currentProjectSlug: string,
): ArtifactSearchResult[] {
  return [...results].sort((a, b) => {
    const aCurrent = a.projectSlug === currentProjectSlug ? 0 : 1;
    const bCurrent = b.projectSlug === currentProjectSlug ? 0 : 1;
    if (aCurrent !== bCurrent) return aCurrent - bCurrent;
    if (a.projectSlug !== b.projectSlug)
      return a.projectSlug.localeCompare(b.projectSlug);
    return a.artifactName.localeCompare(b.artifactName);
  });
}

function groupedTypes(types: LinkType[]): Array<[string, LinkType[]]> {
  const builtin = types.filter((t) => t.source === "builtin");
  const system = types.filter((t) => t.source === "system");
  const out: Array<[string, LinkType[]]> = [];
  if (builtin.length > 0) out.push(["Built-in", builtin]);
  if (system.length > 0) out.push(["System", system]);
  return out;
}
