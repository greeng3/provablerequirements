import { useEffect, useState, type FormEvent } from "react";

import { useCreateCollection } from "../../api/queries";

interface Props {
  readonly projectSlug: string;
  readonly onClose: () => void;
}

export function NewCollectionDialog({ projectSlug, onClose }: Props) {
  const mutation = useCreateCollection(projectSlug);
  const [dirName, setDirName] = useState("");
  const [prefix, setPrefix] = useState("");
  const [name, setName] = useState("");
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
        dirName: dirName.trim(),
        prefix: prefix.trim(),
        name: name.trim(),
        description: description.trim() === "" ? null : description.trim(),
      },
      { onSuccess: onClose },
    );
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="new-collection-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="new-collection-heading"
          className="text-lg font-semibold tracking-tight"
        >
          New Collection
        </h2>
        <form onSubmit={submit} className="mt-4 space-y-3">
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Directory name
            </span>
            <input
              required
              value={dirName}
              onChange={(e) => setDirName(e.target.value)}
              pattern="[A-Za-z0-9._\-]+"
              placeholder="requirements"
              className="w-full rounded border border-slate-300 px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
            />
          </label>
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Prefix
            </span>
            <input
              required
              value={prefix}
              onChange={(e) => setPrefix(e.target.value)}
              pattern="[A-Za-z0-9]+"
              placeholder="REQ"
              className="w-full rounded border border-slate-300 px-2 py-1 font-mono text-sm uppercase dark:border-slate-600 dark:bg-slate-800"
            />
            <span className="mt-1 block text-xs text-slate-500">
              Alphanumeric, used at the start of every artifact name (e.g.
              REQ-helloWorld).
            </span>
          </label>
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Display name
            </span>
            <input
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Requirements"
              className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
            />
          </label>
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Description (optional)
            </span>
            <input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
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
              disabled={mutation.isPending || !dirName || !prefix || !name}
              className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
            >
              {mutation.isPending ? "Creating…" : "Create"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
