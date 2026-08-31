import { useEffect, useId, useMemo, useState, type FormEvent } from "react";

import {
  useArtifact,
  useLinkTypes,
  useUpdateArtifact,
} from "../../../api/queries";
import type { GraphNodeDto, LinkWriteRequest } from "../../../api/types";

interface Props {
  readonly source: GraphNodeDto;
  readonly target: GraphNodeDto;
  readonly onClose: () => void;
  readonly onCreated: (linkType: string) => void;
}

/// Minimal link-type picker for drag-to-link authoring (7a.3).
/// Mirrors the existing RenameArtifactDialog shell style; the
/// round-trip posts the source artifact's full links array plus
/// the new entry to PUT /api/artifacts/:uuid, matching what the
/// Phase 3b LinkPicker does from the artifact detail page.
export function LinkCreateDialog({
  source,
  target,
  onClose,
  onCreated,
}: Props) {
  const catalog = useLinkTypes();
  const current = useArtifact(source.uuid);
  const mutation = useUpdateArtifact(source.uuid);
  const selectId = useId();
  const [selected, setSelected] = useState<string>("");

  const options = useMemo(
    () =>
      (catalog.data ?? []).slice().sort((a, b) => a.name.localeCompare(b.name)),
    [catalog.data],
  );

  useEffect(() => {
    if (!selected && options.length > 0) setSelected(options[0].name);
  }, [options, selected]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const duplicate = useMemo(() => {
    const existing = current.data?.links ?? [];
    return existing.some(
      (l) => l.targetUuid === target.uuid && l.type === selected,
    );
  }, [current.data?.links, target.uuid, selected]);

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!selected) return;
    if (!current.data) return;
    if (duplicate) return;
    const existing: LinkWriteRequest[] = current.data.links.map((l) => ({
      targetUuid: l.targetUuid,
      type: l.type,
      hint: l.hint,
    }));
    const nextLinks: LinkWriteRequest[] = [
      ...existing,
      {
        targetUuid: target.uuid,
        type: selected,
        hint: {
          projectSlug: target.projectSlug,
          collectionPrefix: target.collectionPrefix,
          artifactName: target.artifactName,
        },
      },
    ];
    mutation.mutate(
      { links: nextLinks },
      {
        onSuccess: () => {
          onCreated(selected);
          onClose();
        },
      },
    );
  };

  const isLoading = current.isLoading || catalog.isLoading;
  const disableSubmit =
    mutation.isPending || !selected || !current.data || duplicate || isLoading;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="link-create-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="link-create-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Link artifacts
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          <span className="font-mono">
            {source.projectSlug}/{source.collectionPrefix}/{source.artifactName}
          </span>{" "}
          →{" "}
          <span className="font-mono">
            {target.projectSlug}/{target.collectionPrefix}/{target.artifactName}
          </span>
        </p>

        <form onSubmit={submit} className="mt-4 space-y-3">
          <label className="block text-sm">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Link type
            </span>
            <select
              id={selectId}
              value={selected}
              onChange={(e) => setSelected(e.target.value)}
              disabled={isLoading || options.length === 0}
              className="w-full rounded border border-slate-300 bg-white px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
            >
              {options.length === 0 ? (
                <option value="">(loading…)</option>
              ) : (
                options.map((t) => (
                  <option key={t.name} value={t.name}>
                    {t.name}
                    {t.directed ? "" : " (undirected)"}
                    {t.acyclic ? " · acyclic" : ""}
                  </option>
                ))
              )}
            </select>
          </label>

          {duplicate ? (
            <p
              className="text-xs text-amber-700 dark:text-amber-300"
              role="alert"
            >
              A {selected} link from this source to this target already exists.
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
              disabled={disableSubmit}
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
