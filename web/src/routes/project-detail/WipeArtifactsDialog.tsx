import { useEffect, useState } from "react";

import { useWipeProjectArtifacts } from "../../api/queries";

interface Props {
  readonly projectSlug: string;
  readonly onClose: () => void;
}

/// Scorched-earth wipe modal mounted from the Project detail page.
/// Default mode removes every collection directory in the project
/// but leaves reqforge.json + the artifacts/ root in place. The
/// "also de-initialize" checkbox widens the blast radius to remove
/// reqforge.json and the artifacts/ directory itself, reverting
/// the mount to a fresh-clone NeedsInit state. Requires the
/// operator to type the project slug before the action enables.
export function WipeArtifactsDialog({ projectSlug, onClose }: Props) {
  const mutation = useWipeProjectArtifacts(projectSlug);
  const [typed, setTyped] = useState("");
  const [deinit, setDeinit] = useState(false);
  const confirmed = typed.trim() === projectSlug;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const run = () => {
    if (!confirmed) return;
    mutation.mutate(
      { deinit },
      {
        onSuccess: () => onClose(),
      },
    );
  };

  const heading = deinit
    ? "Wipe and de-initialize this Project"
    : "Wipe all artifacts in this Project";
  const confirmLabel = mutation.isPending
    ? deinit
      ? "Wiping + de-initializing…"
      : "Wiping…"
    : deinit
      ? "Wipe and de-initialize"
      : "Wipe artifacts";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="wipe-artifacts-heading"
      data-testid="wipe-artifacts-dialog"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="max-h-[90vh] w-full max-w-lg overflow-auto rounded-lg border border-rose-300 bg-white p-6 shadow-lg dark:border-rose-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="wipe-artifacts-heading"
          className="text-lg font-semibold tracking-tight text-rose-700 dark:text-rose-400"
        >
          {heading}
        </h2>
        <p className="mt-2 text-sm text-slate-700 dark:text-slate-300">
          Deletes every collection directory in{" "}
          <span className="font-mono">{projectSlug}</span> along with all
          artifacts inside them. The Project itself (
          <span className="font-mono">reqforge.json</span>) is left untouched so
          the Project continues to load — but every collection, artifact, and
          link will be gone.
        </p>
        {deinit ? (
          <p
            className="mt-2 text-sm text-rose-700 dark:text-rose-400"
            data-testid="wipe-artifacts-deinit-note"
          >
            With "Also de-initialize" checked,{" "}
            <span className="font-mono">reqforge.json</span> and the{" "}
            <span className="font-mono">artifacts/</span> directory itself are
            also removed, so this repo will revert to a NeedsInit mount as if
            ReqForge had never touched it.
          </p>
        ) : null}
        <p className="mt-2 text-sm font-semibold text-rose-700 dark:text-rose-400">
          This cannot be undone from the UI. Make sure you have a commit (or can{" "}
          <span className="font-mono">git restore</span>) before proceeding.
        </p>

        <label className="mt-4 flex items-start gap-2 text-sm text-slate-700 dark:text-slate-300">
          <input
            type="checkbox"
            checked={deinit}
            onChange={(e) => setDeinit(e.target.checked)}
            data-testid="wipe-artifacts-deinit-checkbox"
            className="mt-1"
          />
          <span>
            Also de-initialize this Project (remove{" "}
            <span className="font-mono">reqforge.json</span> and the{" "}
            <span className="font-mono">artifacts/</span> directory).
          </span>
        </label>

        <label
          htmlFor="wipe-confirm-slug"
          className="mt-5 block text-sm text-slate-700 dark:text-slate-300"
        >
          Type the Project slug{" "}
          <span className="font-mono font-semibold">{projectSlug}</span> to
          confirm:
        </label>
        <input
          id="wipe-confirm-slug"
          type="text"
          autoComplete="off"
          spellCheck={false}
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
          data-testid="wipe-artifacts-confirm-input"
          className="mt-1 w-full rounded border border-slate-300 bg-white px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
          placeholder={projectSlug}
        />

        {mutation.error ? (
          <p
            className="mt-3 text-sm text-rose-600"
            role="alert"
            data-testid="wipe-artifacts-error"
          >
            {String(mutation.error)}
          </p>
        ) : null}

        <div className="mt-6 flex flex-wrap justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={run}
            disabled={!confirmed || mutation.isPending}
            data-testid="wipe-artifacts-confirm"
            className="rounded bg-rose-600 px-3 py-1 text-sm font-semibold text-white hover:bg-rose-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
