import { useEffect, useState, type FormEvent } from "react";

import { useReplaceBlob } from "../../api/queries";
import type { ArtifactDetail } from "../../api/types";

interface Props {
  readonly artifact: ArtifactDetail;
  readonly onClose: () => void;
}

// Implements: ART003 — a blob is updated only by uploading a replacement.
export function ReplaceBlobDialog({ artifact, onClose }: Props) {
  const mutation = useReplaceBlob(artifact.uuid);
  const [file, setFile] = useState<File | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!file) return;
    const form = new FormData();
    form.append("file", file);
    mutation.mutate(form, { onSuccess: () => onClose() });
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="replace-blob-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="replace-blob-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Replace file
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          Replace the binary for{" "}
          <span className="font-mono">{artifact.name}</span>. The UUID, review
          log, and outgoing links are preserved.
        </p>
        <form onSubmit={submit} className="mt-4 space-y-3">
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              New file
            </span>
            <input
              type="file"
              required
              onChange={(e) => setFile(e.target.files?.[0] ?? null)}
              className="block w-full text-sm"
            />
          </label>
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
              disabled={mutation.isPending || !file}
              className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
            >
              {mutation.isPending ? "Uploading…" : "Replace"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
