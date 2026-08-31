import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { useArtifacts, useBulkCheckUrls, useCollection } from "../api/queries";
import { BulkUrlCheckButton } from "./collection-detail/BulkUrlCheckButton";
import { EmptyArtifactsState } from "./collection-detail/EmptyArtifactsState";
import { NewArtifactDialog } from "./collection-detail/NewArtifactDialog";
import { VirtualArtifactList } from "./collection-detail/VirtualArtifactList";

export function CollectionPage() {
  const { slug, prefix } = useParams<{ slug: string; prefix: string }>();
  const collection = useCollection(slug, prefix);
  const artifacts = useArtifacts(slug, prefix);
  const [showNew, setShowNew] = useState(false);
  const artifactList = artifacts.data ?? [];
  const urlArtifactCount = useMemo(
    () => artifactList.filter((a) => a.shape === "url").length,
    [artifactList],
  );
  const bulkCheck = useBulkCheckUrls(slug ?? "", prefix ?? "");

  if (collection.isLoading || artifacts.isLoading) {
    return <p className="text-sm text-slate-500">Loading collection…</p>;
  }
  if (collection.isError || !collection.data) {
    return (
      <p className="text-sm text-rose-600" role="alert">
        Could not load collection {prefix}:{" "}
        {String(collection.error ?? "not found")}
      </p>
    );
  }

  const c = collection.data;

  return (
    <section aria-labelledby="collection-heading" className="space-y-6">
      <header className="flex items-start justify-between gap-4">
        <div>
          <h1
            id="collection-heading"
            className="flex items-baseline gap-2 text-2xl font-semibold tracking-tight"
          >
            <span className="font-mono">{c.prefix}</span>
            <span>{c.name}</span>
          </h1>
          {c.description ? (
            <p className="mt-1 text-slate-600 dark:text-slate-400">
              {c.description}
            </p>
          ) : null}
          <p className="mt-2 text-xs text-slate-500">
            {c.artifactCount} {c.artifactCount === 1 ? "artifact" : "artifacts"}{" "}
            · code-trace expected: {c.expectsCodeTrace ? "yes" : "no"}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {urlArtifactCount > 0 ? (
            <BulkUrlCheckButton
              urlArtifactCount={urlArtifactCount}
              pending={bulkCheck.isPending}
              result={bulkCheck.data}
              error={bulkCheck.error}
              onCheck={() => bulkCheck.mutate(undefined)}
            />
          ) : null}
          <button
            type="button"
            onClick={() => setShowNew(true)}
            className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 dark:bg-slate-100 dark:text-slate-900"
          >
            New artifact
          </button>
        </div>
      </header>

      {artifactList.length > 0 ? (
        <VirtualArtifactList
          projectSlug={slug!}
          prefix={prefix!}
          artifacts={artifactList}
        />
      ) : (
        <EmptyArtifactsState />
      )}

      {showNew && slug && prefix ? (
        <NewArtifactDialog
          projectSlug={slug}
          collectionPrefix={prefix}
          onClose={() => setShowNew(false)}
        />
      ) : null}
    </section>
  );
}
