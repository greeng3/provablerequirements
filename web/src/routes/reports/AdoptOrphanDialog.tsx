import { useEffect, useId, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";

import { useAdoptOrphanBlob } from "../../api/queries";
import type { OrphanBinaryEntry } from "../../api/types";

interface Props {
  readonly orphan: OrphanBinaryEntry;
  readonly onClose: () => void;
}

/// Short wizard — name + title + optional description — that
/// posts back to /artifacts/blob/adopt to materialise a sidecar
/// for an on-disk binary. Reuses the existing upload-form
/// validation style.
export function AdoptOrphanDialog({ orphan, onClose }: Props) {
  const navigate = useNavigate();
  const mutation = useAdoptOrphanBlob(
    orphan.projectSlug,
    orphan.collectionPrefix,
  );
  const nameId = useId();
  const titleId = useId();
  // Seed the name from the on-disk filename stem so operators
  // typically just tweak the title.
  const defaultName = orphan.filename.replace(/\.[^.]+$/, "");
  const [name, setName] = useState(defaultName);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    mutation.mutate(
      {
        name: name.trim(),
        title: title.trim(),
        binaryRelativePath: orphan.binaryRelativePath,
        description: description.trim() || undefined,
      },
      {
        onSuccess: (detail) => {
          navigate(
            `/projects/${detail.projectSlug}/collections/${detail.collectionPrefix}/artifacts/${detail.name}`,
          );
        },
      },
    );
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="adopt-orphan-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="adopt-orphan-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Adopt as artifact
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          Create a sidecar alongside{" "}
          <span className="font-mono">{orphan.binaryRelativePath}</span>. The
          file itself isn't copied; only the `.reqforge.json` sidecar is
          written.
        </p>
        <form onSubmit={submit} className="mt-4 space-y-3">
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Name
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
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Title
            </span>
            <input
              id={titleId}
              required
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
            />
          </label>
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Description (optional)
            </span>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={3}
              className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
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
              disabled={mutation.isPending || !name || !title}
              className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
            >
              {mutation.isPending ? "Adopting…" : "Adopt"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
