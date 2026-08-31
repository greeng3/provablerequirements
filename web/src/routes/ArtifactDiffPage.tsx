import { useEffect } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";

import {
  useArtifact,
  useArtifactDiff,
  useArtifactHistory,
} from "../api/queries";
import type { CommitInfo } from "../api/types";
import { DiffView } from "./artifact-detail/DiffView";

/// Standalone diff route for an artifact. Pickers for `from` and
/// `to` populate from `/api/artifacts/:uuid/history`; a special
/// "working tree" option on the `to` picker maps to the handler's
/// default (omitting `to` on the request URL).
export function ArtifactDiffPage() {
  const { uuid } = useParams<{ uuid: string }>();
  const [params, setParams] = useSearchParams();
  const artifact = useArtifact(uuid);
  const history = useArtifactHistory(uuid);
  const from = params.get("from") ?? undefined;
  const toParam = params.get("to") ?? undefined;
  const diff = useArtifactDiff(uuid, from, toParam);

  // Seed the pickers on first successful history load so the user
  // lands on a useful diff without having to pick manually.
  useEffect(() => {
    if (!from && history.data?.commits?.length) {
      const newest = history.data.commits[0]?.oid;
      if (newest) {
        const next = new URLSearchParams(params);
        next.set("from", newest);
        setParams(next, { replace: true });
      }
    }
  }, [history.data, from, params, setParams]);

  const onSelect = (field: "from" | "to", value: string) => {
    const next = new URLSearchParams(params);
    if (value) next.set(field, value);
    else next.delete(field);
    setParams(next, { replace: true });
  };

  return (
    <section aria-labelledby="diff-heading" className="space-y-4">
      <header className="space-y-1">
        <h1 id="diff-heading" className="text-2xl font-semibold tracking-tight">
          Diff · {artifact.data?.title ?? uuid}
        </h1>
        {uuid && artifact.data ? (
          <BreadcrumbLink artifact={artifact.data} uuid={uuid} />
        ) : null}
      </header>

      <div className="flex flex-wrap items-center gap-3 rounded border border-slate-200 p-3 text-sm dark:border-slate-800">
        <CommitPicker
          label="From"
          value={from ?? ""}
          onChange={(v) => onSelect("from", v)}
          commits={history.data?.commits ?? []}
          includeWorkingTree={false}
        />
        <CommitPicker
          label="To"
          value={toParam ?? ""}
          onChange={(v) => onSelect("to", v)}
          commits={history.data?.commits ?? []}
          includeWorkingTree={true}
        />
      </div>

      {history.data?.fallbackReason ? (
        <p className="rounded border border-amber-300 bg-amber-50 p-2 text-xs text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100">
          History unavailable: {history.data.fallbackReason}
        </p>
      ) : null}

      {!from ? (
        <p className="text-sm text-slate-500">
          Pick a source commit to compare against.
        </p>
      ) : diff.isLoading ? (
        <p className="text-sm text-slate-500">Loading diff…</p>
      ) : diff.isError || !diff.data ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load diff: {String(diff.error ?? "unknown")}
        </p>
      ) : (
        <DiffView response={diff.data} />
      )}
    </section>
  );
}

function BreadcrumbLink({
  artifact,
  uuid,
}: {
  artifact: { projectSlug: string; collectionPrefix: string; name: string };
  uuid: string;
}) {
  return (
    <p className="text-xs text-slate-500">
      <Link
        to={`/projects/${artifact.projectSlug}/collections/${artifact.collectionPrefix}/artifacts/${artifact.name}`}
        className="hover:underline"
      >
        ← Back to {artifact.name}
      </Link>
      <span className="ml-2 font-mono">{uuid}</span>
    </p>
  );
}

function CommitPicker({
  label,
  value,
  onChange,
  commits,
  includeWorkingTree,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  commits: CommitInfo[];
  includeWorkingTree: boolean;
}) {
  return (
    <label className="flex items-center gap-2">
      <span className="text-xs uppercase tracking-wide text-slate-500">
        {label}
      </span>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="rounded border border-slate-300 bg-white px-2 py-1 text-xs dark:border-slate-600 dark:bg-slate-800"
      >
        {!value ? <option value="">— pick a commit —</option> : null}
        {includeWorkingTree ? <option value="">working tree</option> : null}
        {commits.map((c) => (
          <option key={c.oid} value={c.oid}>
            {c.shortOid} · {c.summary}
          </option>
        ))}
      </select>
    </label>
  );
}
