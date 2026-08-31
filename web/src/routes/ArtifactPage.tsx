import { useState } from "react";
import { useParams } from "react-router-dom";

import { useArtifact, useArtifacts } from "../api/queries";
import { ArtifactEditor } from "./artifact-detail/ArtifactEditor";
import { ArtifactView } from "./artifact-detail/ArtifactView";
import { DeleteArtifactDialog } from "./artifact-detail/DeleteArtifactDialog";
import { RenameArtifactDialog } from "./artifact-detail/RenameArtifactDialog";

export function ArtifactPage() {
  const params = useParams<{
    slug?: string;
    prefix?: string;
    name?: string;
    uuid?: string;
  }>();

  const list = useArtifacts(
    params.uuid ? undefined : params.slug,
    params.uuid ? undefined : params.prefix,
  );
  const resolvedUuid =
    params.uuid ?? list.data?.find((a) => a.name === params.name)?.uuid;
  const detail = useArtifact(resolvedUuid);

  const [mode, setMode] = useState<"view" | "edit">("view");
  const [showDelete, setShowDelete] = useState(false);
  const [showRename, setShowRename] = useState(false);

  if (detail.isLoading || (!params.uuid && list.isLoading)) {
    return <p className="text-sm text-slate-500">Loading artifact…</p>;
  }
  if (!resolvedUuid && !list.isLoading) {
    return (
      <p className="text-sm text-rose-600" role="alert">
        Artifact {params.name} not found in {params.prefix}.
      </p>
    );
  }
  if (detail.isError || !detail.data) {
    return (
      <p className="text-sm text-rose-600" role="alert">
        Could not load artifact: {String(detail.error ?? "not found")}
      </p>
    );
  }

  const artifact = detail.data;

  if (mode === "edit" && artifact.shape === "content") {
    return (
      <ArtifactEditor artifact={artifact} onDone={() => setMode("view")} />
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex justify-end gap-2">
        {artifact.shape === "content" ? (
          <button
            type="button"
            onClick={() => setMode("edit")}
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Edit
          </button>
        ) : null}
        <button
          type="button"
          onClick={() => setShowRename(true)}
          className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          Rename
        </button>
        <button
          type="button"
          onClick={() => setShowDelete(true)}
          className="rounded border border-rose-300 px-3 py-1 text-sm text-rose-700 hover:bg-rose-50 dark:border-rose-700 dark:text-rose-300 dark:hover:bg-rose-950"
        >
          Delete
        </button>
      </div>
      <ArtifactView artifact={artifact} />
      {showRename ? (
        <RenameArtifactDialog
          artifact={artifact}
          onClose={() => setShowRename(false)}
        />
      ) : null}
      {showDelete ? (
        <DeleteArtifactDialog
          artifact={artifact}
          onClose={() => setShowDelete(false)}
        />
      ) : null}
    </div>
  );
}
