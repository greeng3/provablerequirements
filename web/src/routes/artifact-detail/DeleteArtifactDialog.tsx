import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

import { useDeleteArtifact, useIncomingLinks } from "../../api/queries";
import type { ArtifactDetail } from "../../api/types";

interface Props {
  readonly artifact: ArtifactDetail;
  readonly onClose: () => void;
}

export function DeleteArtifactDialog({ artifact, onClose }: Props) {
  const navigate = useNavigate();
  const incoming = useIncomingLinks(artifact.uuid);
  const deleteMut = useDeleteArtifact();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const confirm = () => {
    deleteMut.mutate(artifact.uuid, {
      onSuccess: () => {
        navigate(
          `/projects/${artifact.projectSlug}/collections/${artifact.collectionPrefix}`,
        );
      },
    });
  };

  const count = incoming.data?.length ?? 0;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="delete-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="delete-heading"
          className="text-lg font-semibold tracking-tight text-rose-700 dark:text-rose-300"
        >
          Delete {artifact.name}?
        </h2>
        <p className="mt-2 text-sm text-slate-700 dark:text-slate-300">
          This removes the artifact file from disk. The action can't be undone
          from within ReqForge — use git to recover.
        </p>

        {incoming.isLoading ? (
          <p className="mt-3 text-sm text-slate-500">
            Checking incoming links…
          </p>
        ) : count > 0 ? (
          <div className="mt-3 rounded border border-amber-300 bg-amber-50 p-3 text-sm text-amber-900 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-100">
            <p className="font-medium">
              {count} other artifact
              {count === 1 ? "" : "s"} link
              {count === 1 ? "s" : ""} to this one. Proceeding will leave those
              links unresolved (ReqForge does not auto-rewrite sources).
            </p>
            <ul className="mt-2 space-y-0.5 font-mono text-xs">
              {(incoming.data ?? []).slice(0, 10).map((link) => (
                <li key={`${link.sourceUuid}:${link.linkType}`}>
                  {link.projectSlug}/{link.collectionPrefix}/{link.artifactName}{" "}
                  <span className="text-amber-700 dark:text-amber-300">
                    ({link.linkType})
                  </span>
                </li>
              ))}
              {count > 10 ? (
                <li className="text-amber-700 dark:text-amber-300">
                  …and {count - 10} more.
                </li>
              ) : null}
            </ul>
          </div>
        ) : (
          <p className="mt-3 text-sm text-slate-500">No incoming links.</p>
        )}

        {deleteMut.error ? (
          <p className="mt-3 text-sm text-rose-600" role="alert">
            {String(deleteMut.error)}
          </p>
        ) : null}

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={confirm}
            disabled={deleteMut.isPending}
            className="rounded bg-rose-600 px-3 py-1 text-sm text-white hover:bg-rose-500 disabled:opacity-50"
          >
            {deleteMut.isPending ? "Deleting…" : "Delete"}
          </button>
        </div>
      </div>
    </div>
  );
}
