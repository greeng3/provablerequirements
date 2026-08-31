import type { ArtifactDetail } from "../../api/types";
import { ArtifactMarkdown } from "./ArtifactMarkdown";
import { BlobArtifactView } from "./BlobArtifactView";
import { OutgoingLinks } from "./OutgoingLinks";
import { ReviewPane } from "./ReviewPane";
import { TagList } from "./TagList";
import { UrlArtifactView } from "./UrlArtifactView";

export function ArtifactView({ artifact }: { artifact: ArtifactDetail }) {
  return (
    <section aria-labelledby="artifact-heading" className="space-y-6">
      <header>
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-sm text-slate-500">
            {artifact.name}
          </span>
          {artifact.outlineLevel ? (
            <span
              className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-xs text-slate-600 dark:bg-slate-800 dark:text-slate-400"
              aria-label={`outline level ${artifact.outlineLevel}`}
            >
              {artifact.outlineLevel}
            </span>
          ) : null}
          {!artifact.active ? (
            <span className="rounded bg-slate-200 px-1.5 py-0.5 text-xs text-slate-700 dark:bg-slate-700 dark:text-slate-200">
              inactive
            </span>
          ) : null}
          {artifact.derived ? (
            <span className="rounded bg-amber-100 px-1.5 py-0.5 text-xs text-amber-800 dark:bg-amber-900/50 dark:text-amber-200">
              derived
            </span>
          ) : null}
        </div>
        <h1
          id="artifact-heading"
          className="mt-1 text-2xl font-semibold tracking-tight"
        >
          {artifact.title}
        </h1>
        {artifact.description ? (
          <p className="mt-1 text-slate-600 dark:text-slate-400">
            {artifact.description}
          </p>
        ) : null}
        <TagList tags={artifact.tags} />
      </header>

      <div className="grid gap-6 lg:grid-cols-[1fr_18rem]">
        <div>
          {artifact.shape === "content" ? (
            <ArtifactMarkdown body={artifact.body} />
          ) : artifact.shape === "blob" ? (
            <BlobArtifactView artifact={artifact} />
          ) : (
            <UrlArtifactView artifact={artifact} />
          )}
        </div>
        <aside className="space-y-6 rounded-lg border border-slate-200 bg-white p-4 text-sm dark:border-slate-800 dark:bg-slate-900">
          <section aria-labelledby="links-heading">
            <h2
              id="links-heading"
              className="mb-2 text-sm font-semibold tracking-wide text-slate-700 dark:text-slate-300"
            >
              Outgoing links ({artifact.links.length})
            </h2>
            <OutgoingLinks links={artifact.links} />
          </section>
          <ReviewPane artifact={artifact} />
        </aside>
      </div>
    </section>
  );
}
