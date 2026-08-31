import { useCallback, useId, useRef, useState } from "react";

import { MarkdownEditor } from "../../components/MarkdownEditor";
import type {
  ArtifactDetail,
  LinkView,
  LinkWriteRequest,
} from "../../api/types";
import { useUpdateArtifact } from "../../api/queries";
import { ConflictDialog, type ConflictChoice } from "./ConflictDialog";
import { LinkPicker } from "./LinkPicker";
import { OutgoingLinks } from "./OutgoingLinks";

interface Props {
  readonly artifact: ArtifactDetail;
  readonly onDone: () => void;
}

/// Edit mode for a content-hosted artifact: title, description,
/// tags, active flag, typed outgoing links, and body (via the
/// CodeMirror + preview MarkdownEditor). Save dispatches a PUT;
/// Discard bails without writing.
export function ArtifactEditor({ artifact, onDone }: Props) {
  const titleId = useId();
  const descriptionId = useId();
  const tagsId = useId();
  const outlineLevelId = useId();
  const mutation = useUpdateArtifact(artifact.uuid);

  const [title, setTitle] = useState(artifact.title);
  const [description, setDescription] = useState(artifact.description ?? "");
  const [tagsText, setTagsText] = useState(artifact.tags.join(", "));
  const [active, setActive] = useState(artifact.active);
  const [derived, setDerived] = useState(artifact.derived);
  const [outlineLevel, setOutlineLevel] = useState(artifact.outlineLevel ?? "");
  const [body, setBody] = useState(artifact.body ?? "");
  const [stagedLinks, setStagedLinks] = useState<LinkView[]>(artifact.links);
  const [pickerOpen, setPickerOpen] = useState(false);

  // Latch the modifiedAt the editor opened against. If the
  // artifact prop's modifiedAt drifts (the watcher / SSE detected
  // an external change), we pop a conflict modal.
  const loadedModifiedAt = useRef(artifact.modifiedAt);
  const loadedBody = useRef(artifact.body ?? "");
  const loadedLinksKey = useRef(linksKey(artifact.links));
  const hasLocalEdits =
    body !== loadedBody.current ||
    title !== artifact.title ||
    description !== (artifact.description ?? "") ||
    tagsText !== artifact.tags.join(", ") ||
    active !== artifact.active ||
    derived !== artifact.derived ||
    outlineLevel !== (artifact.outlineLevel ?? "") ||
    linksKey(stagedLinks) !== loadedLinksKey.current;
  const externalEdit = artifact.modifiedAt !== loadedModifiedAt.current;
  const [conflictDismissed, setConflictDismissed] = useState(false);
  const showConflict = externalEdit && hasLocalEdits && !conflictDismissed;

  const resolveConflict = (choice: ConflictChoice) => {
    if (choice.kind === "discard") {
      setBody(artifact.body ?? "");
      setTitle(artifact.title);
      setDescription(artifact.description ?? "");
      setTagsText(artifact.tags.join(", "));
      setActive(artifact.active);
      setDerived(artifact.derived);
      setOutlineLevel(artifact.outlineLevel ?? "");
      setStagedLinks(artifact.links);
    } else if (choice.kind === "merged") {
      setBody(choice.body);
    }
    loadedModifiedAt.current = artifact.modifiedAt;
    loadedBody.current =
      choice.kind === "merged" ? choice.body : (artifact.body ?? "");
    loadedLinksKey.current = linksKey(artifact.links);
    setConflictDismissed(true);
  };

  const save = useCallback(() => {
    const tags = tagsText
      .split(",")
      .map((t) => t.trim())
      .filter((t) => t.length > 0);
    const linksPayload: LinkWriteRequest[] = stagedLinks.map((l) => ({
      targetUuid: l.targetUuid,
      type: l.type,
      hint: l.hint,
    }));
    mutation.mutate(
      {
        title,
        description: description.trim() === "" ? null : description,
        tags,
        active,
        derived,
        outlineLevel: outlineLevel.trim() === "" ? null : outlineLevel.trim(),
        body,
        links: linksPayload,
      },
      { onSuccess: onDone },
    );
  }, [
    mutation,
    title,
    description,
    tagsText,
    active,
    derived,
    outlineLevel,
    body,
    stagedLinks,
    onDone,
  ]);

  const addLink = (req: LinkWriteRequest) => {
    // Append as an optimistic LinkView — resolution/typeMetadata/
    // targetSummary will be filled in authoritatively when the
    // server's PUT response lands and react-query refreshes.
    setStagedLinks((current) => [
      ...current,
      {
        targetUuid: req.targetUuid,
        type: req.type,
        hint: req.hint ?? {
          projectSlug: "",
          collectionPrefix: "",
          artifactName: "",
        },
        resolution: "resolved",
      },
    ]);
  };

  const removeLink = (index: number) => {
    setStagedLinks((current) => current.filter((_, i) => i !== index));
  };

  return (
    <section aria-labelledby="artifact-heading" className="space-y-4">
      <header>
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-sm text-slate-500">
            {artifact.name}
          </span>
          <span className="rounded bg-amber-100 px-1.5 py-0.5 text-xs text-amber-800 dark:bg-amber-900/50 dark:text-amber-200">
            editing
          </span>
        </div>
        <h1
          id="artifact-heading"
          className="mt-1 text-2xl font-semibold tracking-tight"
        >
          Editing {artifact.name}
        </h1>
      </header>

      <div className="grid gap-3">
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            Title
          </span>
          <input
            id={titleId}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            Description
          </span>
          <input
            id={descriptionId}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            Tags (comma-separated)
          </span>
          <input
            id={tagsId}
            value={tagsText}
            onChange={(e) => setTagsText(e.target.value)}
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            Outline level (optional)
          </span>
          <input
            id={outlineLevelId}
            value={outlineLevel}
            onChange={(e) => setOutlineLevel(e.target.value)}
            placeholder="e.g. 1.2.3"
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <div className="flex gap-6">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={active}
              onChange={(e) => setActive(e.target.checked)}
            />
            <span>Active</span>
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={derived}
              onChange={(e) => setDerived(e.target.checked)}
            />
            <span>Derived</span>
          </label>
        </div>
      </div>

      <section aria-labelledby="links-heading" className="space-y-2">
        <div className="flex items-center justify-between">
          <h2
            id="links-heading"
            className="text-sm font-semibold text-slate-700 dark:text-slate-200"
          >
            Outgoing links
          </h2>
          <button
            type="button"
            onClick={() => setPickerOpen(true)}
            className="rounded border border-slate-300 px-2 py-0.5 text-xs hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Add link
          </button>
        </div>
        <OutgoingLinks links={stagedLinks} onRemove={removeLink} />
      </section>

      <MarkdownEditor
        value={body}
        onChange={setBody}
        ariaLabel="Artifact body"
      />

      {mutation.error ? (
        <p className="text-sm text-rose-600" role="alert">
          {String(mutation.error)}
        </p>
      ) : null}

      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onDone}
          className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          Discard
        </button>
        <button
          type="button"
          onClick={save}
          disabled={mutation.isPending}
          className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
        >
          {mutation.isPending ? "Saving…" : "Save"}
        </button>
      </div>

      {pickerOpen ? (
        <LinkPicker
          currentArtifactUuid={artifact.uuid}
          currentProjectSlug={artifact.projectSlug}
          onCommit={addLink}
          onClose={() => setPickerOpen(false)}
        />
      ) : null}

      {showConflict ? (
        <ConflictDialog
          localBody={body}
          externalBody={artifact.body ?? ""}
          onResolve={resolveConflict}
        />
      ) : null}
    </section>
  );
}

function linksKey(links: LinkView[]): string {
  return links.map((l) => `${l.type}:${l.targetUuid}`).join("|");
}
