import { useEffect, useId, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";

import {
  useLlmProviders,
  useRenameArtifact,
  useRenameSuggestions,
} from "../../api/queries";
import type {
  ArtifactDetail,
  RenameSuggestion,
  RenameSuggestionsResponse,
} from "../../api/types";

interface Props {
  readonly artifact: ArtifactDetail;
  readonly onClose: () => void;
}

export function RenameArtifactDialog({ artifact, onClose }: Props) {
  const navigate = useNavigate();
  const mutation = useRenameArtifact(artifact.uuid);
  const llmProviders = useLlmProviders();
  const suggestMutation = useRenameSuggestions();
  const nameId = useId();
  const [name, setName] = useState(artifact.name);

  const hasLlm = (llmProviders.data?.providers?.length ?? 0) > 0;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const next = name.trim();
    if (next === artifact.name) {
      onClose();
      return;
    }
    mutation.mutate(
      { name: next },
      {
        onSuccess: () => {
          navigate(
            `/projects/${artifact.projectSlug}/collections/${artifact.collectionPrefix}/artifacts/${next}`,
          );
        },
      },
    );
  };

  const onSuggest = () => suggestMutation.mutate(artifact.uuid);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="rename-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="rename-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Rename {artifact.name}
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          UUID stays the same, so incoming links keep resolving automatically.
          The filename on disk changes to match.
        </p>
        <form onSubmit={submit} className="mt-4 space-y-3">
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              New name
            </span>
            <input
              id={nameId}
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
              pattern="[A-Za-z0-9._\-]+"
              className="w-full rounded border border-slate-300 px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
            />
          </label>
          {hasLlm ? (
            <SuggestPanel
              onSuggest={onSuggest}
              onPick={(suggestion) => setName(suggestion.name)}
              isPending={suggestMutation.isPending}
              data={suggestMutation.data}
              error={suggestMutation.error}
            />
          ) : null}
          {mutation.error ? (
            <p className="text-sm text-rose-600" role="alert">
              {String(mutation.error)}
            </p>
          ) : null}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={mutation.isPending || !name}
              className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
            >
              {mutation.isPending ? "Renaming…" : "Rename"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

interface SuggestPanelProps {
  readonly onSuggest: () => void;
  readonly onPick: (suggestion: RenameSuggestion) => void;
  readonly isPending: boolean;
  readonly data?: RenameSuggestionsResponse;
  readonly error: Error | null;
}

function SuggestPanel({
  onSuggest,
  onPick,
  isPending,
  data,
  error,
}: SuggestPanelProps) {
  return (
    <div
      data-testid="rename-suggest-panel"
      className="rounded border border-slate-200 bg-slate-50 p-2 text-xs dark:border-slate-700 dark:bg-slate-800/60"
    >
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium text-slate-600 dark:text-slate-300">
          LLM suggestions
        </span>
        <button
          type="button"
          onClick={onSuggest}
          disabled={isPending}
          data-testid="rename-suggest-button"
          className="rounded border border-slate-300 bg-white px-2 py-0.5 text-xs hover:bg-slate-100 disabled:opacity-50 dark:border-slate-600 dark:bg-slate-900 dark:hover:bg-slate-800"
        >
          {isPending ? "Asking…" : "Suggest names"}
        </button>
      </div>
      {error ? (
        <p className="mt-2 text-rose-600" role="alert">
          {String(error)}
        </p>
      ) : data?.kind === "privacyAckRequired" ? (
        <p
          className="mt-2 text-amber-700 dark:text-amber-300"
          role="alert"
          data-testid="rename-suggest-privacy-alert"
        >
          Privacy warning not yet acknowledged for provider
          {data.indices.length === 1 ? "" : "s"} {data.indices.join(", ")}.{" "}
          <a
            href="/llm"
            className="underline underline-offset-2 hover:text-amber-900 dark:hover:text-amber-100"
          >
            Acknowledge in LLM providers →
          </a>
        </p>
      ) : data?.kind === "noProviders" ? (
        <p className="mt-2 text-slate-500">No LLM providers are configured.</p>
      ) : data?.kind === "ok" ? (
        <>
          <ul data-testid="rename-suggest-list" className="mt-2 space-y-1">
            {data.suggestions.map((s) => (
              <li key={s.name} className="flex flex-wrap items-baseline gap-2">
                <button
                  type="button"
                  onClick={() => onPick(s)}
                  data-testid={`rename-suggest-pick-${s.name}`}
                  className="rounded border border-slate-300 bg-white px-2 py-0.5 font-mono text-xs hover:bg-slate-100 dark:border-slate-600 dark:bg-slate-900 dark:hover:bg-slate-800"
                >
                  {s.name}
                </button>
                <span className="text-slate-600 dark:text-slate-400">
                  {s.rationale}
                </span>
              </li>
            ))}
          </ul>
          <p className="mt-2 text-slate-500">
            Served by <span className="font-mono">{data.servedBy}</span>.
          </p>
        </>
      ) : null}
    </div>
  );
}
