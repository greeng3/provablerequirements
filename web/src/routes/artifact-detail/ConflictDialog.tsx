import { useEffect, useState } from "react";

export type ConflictChoice =
  | { kind: "keep" }
  | { kind: "discard" }
  | { kind: "merged"; body: string };

interface Props {
  readonly localBody: string;
  readonly externalBody: string;
  readonly onResolve: (choice: ConflictChoice) => void;
}

/// External-edit conflict prompt per UX-externalEditConflict. The
/// spec demands three options and no auto-dismiss. The dialog
/// stays modal until the user picks one.
export function ConflictDialog({ localBody, externalBody, onResolve }: Props) {
  const [showMerge, setShowMerge] = useState(false);
  const [mergedBody, setMergedBody] = useState(localBody);

  useEffect(() => {
    // Do NOT bind Escape to a dismissal: the spec forbids
    // auto-dismiss. The user must pick one of the three options.
  }, []);

  if (showMerge) {
    return (
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="conflict-heading"
        className="fixed inset-0 z-20 flex items-center justify-center bg-black/60 p-4"
      >
        <div className="w-full max-w-6xl rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900">
          <h2
            id="conflict-heading"
            className="text-lg font-semibold tracking-tight text-amber-700 dark:text-amber-300"
          >
            Resolve merge manually
          </h2>
          <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
            Edit the merged pane on the right to produce the final saved body.
            Copy content from the left two panes as needed.
          </p>
          <div className="mt-4 grid gap-3 lg:grid-cols-3">
            <MergePane label="Your changes" value={localBody} readOnly />
            <MergePane label="External version" value={externalBody} readOnly />
            <MergePane
              label="Merged result"
              value={mergedBody}
              onChange={setMergedBody}
            />
          </div>
          <div className="mt-4 flex justify-between gap-2">
            <button
              type="button"
              onClick={() => setShowMerge(false)}
              className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
            >
              ← Back
            </button>
            <button
              type="button"
              onClick={() => onResolve({ kind: "merged", body: mergedBody })}
              className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 dark:bg-slate-100 dark:text-slate-900"
            >
              Save merged
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="conflict-heading"
      className="fixed inset-0 z-20 flex items-center justify-center bg-black/60 p-4"
    >
      <div className="w-full max-w-md rounded-lg border border-amber-300 bg-white p-6 shadow-lg dark:border-amber-700 dark:bg-slate-900">
        <h2
          id="conflict-heading"
          className="text-lg font-semibold tracking-tight text-amber-700 dark:text-amber-300"
        >
          External edit detected
        </h2>
        <p className="mt-2 text-sm text-slate-700 dark:text-slate-300">
          This artifact was modified on disk while you were editing it — maybe
          by a <span className="font-mono">git pull</span> or a text-editor save
          outside ReqForge. You have unsaved changes. How do you want to
          proceed?
        </p>
        <ul className="mt-4 space-y-2 text-sm">
          <li>
            <button
              type="button"
              onClick={() => onResolve({ kind: "keep" })}
              className="block w-full rounded border border-slate-300 px-3 py-2 text-left hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
            >
              <strong>Keep my changes.</strong> The external update is abandoned
              in the UI; your next Save overwrites it.
            </button>
          </li>
          <li>
            <button
              type="button"
              onClick={() => onResolve({ kind: "discard" })}
              className="block w-full rounded border border-slate-300 px-3 py-2 text-left hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
            >
              <strong>Discard my changes and reload.</strong> Replaces the
              in-memory state with the external version; your unsaved edits are
              lost.
            </button>
          </li>
          <li>
            <button
              type="button"
              onClick={() => setShowMerge(true)}
              className="block w-full rounded border border-slate-300 px-3 py-2 text-left hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
            >
              <strong>Open merge diff.</strong> Three-pane view with your
              version, the external version, and an editable merged pane to
              produce the final saved body.
            </button>
          </li>
        </ul>
      </div>
    </div>
  );
}

function MergePane({
  label,
  value,
  readOnly,
  onChange,
}: {
  label: string;
  value: string;
  readOnly?: boolean;
  onChange?: (next: string) => void;
}) {
  return (
    <label className="flex flex-col text-sm">
      <span className="mb-1 font-medium text-slate-700 dark:text-slate-200">
        {label}
      </span>
      <textarea
        value={value}
        readOnly={readOnly}
        onChange={onChange ? (e) => onChange(e.target.value) : undefined}
        className="h-80 w-full resize-none rounded border border-slate-300 bg-white px-2 py-1 font-mono text-xs dark:border-slate-600 dark:bg-slate-800"
        aria-label={label}
      />
    </label>
  );
}
