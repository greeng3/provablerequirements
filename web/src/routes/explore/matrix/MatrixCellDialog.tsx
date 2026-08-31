import { useEffect, type FormEvent } from "react";

import { useArtifact, useUpdateArtifact } from "../../../api/queries";
import type { LinkWriteRequest, MatrixNodeDto } from "../../../api/types";

interface Props {
  readonly row: MatrixNodeDto;
  readonly column: MatrixNodeDto;
  readonly linkType: string;
  /// Current-cell state at open time. The backend edge set has
  /// already been consulted; passing the state in keeps the
  /// dialog from having to re-derive it from a fresh fetch.
  readonly initialFilled: boolean;
  readonly onClose: () => void;
  readonly onToggled: (action: "created" | "removed") => void;
}

/// Confirmation modal for the click-to-toggle authoring path
/// (7b.3). Click an empty cell → Create dialog; click a filled
/// cell → Remove dialog. Both rewrite the row artifact's full
/// links array through the existing `PUT /api/artifacts/:uuid`
/// mutation — same round-trip the Phase 7a drag-to-link uses.
export function MatrixCellDialog({
  row,
  column,
  linkType,
  initialFilled,
  onClose,
  onToggled,
}: Props) {
  const current = useArtifact(row.uuid);
  const mutation = useUpdateArtifact(row.uuid);
  const action: "create" | "remove" = initialFilled ? "remove" : "create";

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!current.data) return;
    const existing: LinkWriteRequest[] = current.data.links.map((l) => ({
      targetUuid: l.targetUuid,
      type: l.type,
      hint: l.hint,
    }));
    const nextLinks: LinkWriteRequest[] =
      action === "create"
        ? [
            ...existing,
            {
              targetUuid: column.uuid,
              type: linkType,
              hint: {
                projectSlug: column.projectSlug,
                collectionPrefix: column.collectionPrefix,
                artifactName: column.artifactName,
              },
            },
          ]
        : existing.filter(
            (l) => !(l.targetUuid === column.uuid && l.type === linkType),
          );
    mutation.mutate(
      { links: nextLinks },
      {
        onSuccess: () => {
          onToggled(action === "create" ? "created" : "removed");
          onClose();
        },
      },
    );
  };

  const heading = action === "create" ? "Create link" : "Remove link";
  const prose =
    action === "create"
      ? `Create a ${linkType} link from this row artifact to this column artifact.`
      : `Remove the ${linkType} link from this row artifact to this column artifact.`;
  const cta = action === "create" ? "Create" : "Remove";
  const busyCta = action === "create" ? "Creating…" : "Removing…";
  const ctaPalette =
    action === "create"
      ? "bg-slate-900 hover:bg-slate-700 dark:bg-slate-100 dark:text-slate-900"
      : "bg-rose-700 hover:bg-rose-600 text-white";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="matrix-cell-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="matrix-cell-heading"
          className="text-lg font-semibold tracking-tight"
        >
          {heading}
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          {prose}
        </p>
        <p className="mt-3 text-sm text-slate-700 dark:text-slate-200">
          <span className="font-mono">
            {row.projectSlug}/{row.collectionPrefix}/{row.artifactName}
          </span>{" "}
          <span className="text-slate-500">{linkType}</span>{" "}
          <span className="font-mono">
            {column.projectSlug}/{column.collectionPrefix}/{column.artifactName}
          </span>
        </p>

        <form onSubmit={submit} className="mt-4 space-y-3">
          {current.isLoading ? (
            <p className="text-xs text-slate-500">Loading row artifact…</p>
          ) : current.isError ? (
            <p className="text-sm text-rose-600" role="alert">
              Failed to load row artifact: {String(current.error ?? "unknown")}
            </p>
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
              disabled={
                mutation.isPending || current.isLoading || !current.data
              }
              className={`rounded px-3 py-1 text-sm text-white disabled:opacity-50 ${ctaPalette}`}
            >
              {mutation.isPending ? busyCta : cta}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
