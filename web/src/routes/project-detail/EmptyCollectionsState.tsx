import { useState } from "react";

import { ApiError } from "../../api/client";
import { useCreateSampleContent } from "../../api/queries";

interface Props {
  /// Project slug, so the empty-state can offer an immediate
  /// "Create sample content" action for operators who skipped
  /// the post-init choice screen.
  readonly projectSlug: string;
}

/// Empty-state per UX-emptyStates: "Nothing exists yet" for a
/// project with no Collections. Phase 11b: offer a one-click
/// "Create sample content" action alongside the guidance for
/// operators who skipped the post-init choice.
export function EmptyCollectionsState({ projectSlug }: Props) {
  const sample = useCreateSampleContent(projectSlug);
  const [error, setError] = useState<string | null>(null);

  const run = () => {
    setError(null);
    sample.mutate(undefined, {
      onError: (err) => {
        if (err instanceof ApiError && err.status === 409) {
          const body = err.body as { error?: string } | undefined;
          setError(body?.error ?? "Project is no longer empty.");
        } else {
          setError(String(err));
        }
      },
    });
  };

  return (
    <div className="rounded-lg border border-dashed border-slate-300 bg-white p-6 text-sm text-slate-700 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300">
      <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">
        No Collections yet
      </h2>
      <p className="mt-2">
        This project is empty. Create a Collection via the "New collection"
        button above, or seed the project with a small Task Tracker demo so you
        can explore the traceability graph, reports, and review queue before
        adding your own content.
      </p>
      <div className="mt-4 flex flex-wrap gap-2">
        <button
          type="button"
          onClick={run}
          disabled={sample.isPending}
          data-testid="empty-state-sample-content"
          className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 disabled:opacity-50 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          {sample.isPending ? "Seeding…" : "Create sample content"}
        </button>
      </div>
      {error ? (
        <p
          className="mt-3 text-xs text-rose-600"
          role="alert"
          data-testid="empty-state-sample-error"
        >
          {error}
        </p>
      ) : null}
    </div>
  );
}
