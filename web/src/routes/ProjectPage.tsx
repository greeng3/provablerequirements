import { useState } from "react";
import { useParams } from "react-router-dom";

import { useProject } from "../api/queries";
import { CollectionCard } from "./project-detail/CollectionCard";
import { DoorstopImportDialog } from "./project-detail/DoorstopImportDialog";
import { EmptyCollectionsState } from "./project-detail/EmptyCollectionsState";
import { MigrateSchemaDialog } from "./project-detail/MigrateSchemaDialog";
import { NewCollectionDialog } from "./project-detail/NewCollectionDialog";
import { SchemaDiagnosticsBanner } from "./project-detail/SchemaDiagnosticsBanner";
import { SuggestedLinksTab } from "./project-detail/SuggestedLinksTab";
import { WipeArtifactsDialog } from "./project-detail/WipeArtifactsDialog";

type ProjectTab = "collections" | "suggestions";

export function ProjectPage() {
  const { slug } = useParams<{ slug: string }>();
  const project = useProject(slug);
  const [showNew, setShowNew] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [showMigrate, setShowMigrate] = useState(false);
  const [showWipe, setShowWipe] = useState(false);
  const [tab, setTab] = useState<ProjectTab>("collections");

  if (project.isLoading) {
    return <p className="text-sm text-slate-500">Loading project…</p>;
  }
  if (project.isError || !project.data) {
    return (
      <p className="text-sm text-rose-600" role="alert">
        Could not load project {slug}: {String(project.error ?? "not found")}
      </p>
    );
  }

  const { data } = project;
  return (
    <section aria-labelledby="project-heading" className="space-y-6">
      <header className="flex items-start justify-between gap-4">
        <div>
          <h1
            id="project-heading"
            className="text-2xl font-semibold tracking-tight"
          >
            {data.name}
          </h1>
          {data.description ? (
            <p className="mt-1 text-slate-600 dark:text-slate-400">
              {data.description}
            </p>
          ) : null}
          <p className="mt-2 font-mono text-xs text-slate-500">
            slug: {data.slug} · artifacts path: {data.artifactsPath}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <button
            type="button"
            onClick={() => setShowMigrate(true)}
            data-testid="migrate-schema-open"
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Migrate schema
          </button>
          <button
            type="button"
            onClick={() => setShowImport(true)}
            data-testid="doorstop-import-open"
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Import from doorstop
          </button>
          <button
            type="button"
            onClick={() => setShowWipe(true)}
            data-testid="wipe-artifacts-open"
            className="rounded border border-rose-400 px-3 py-1 text-sm font-bold text-rose-700 hover:bg-rose-50 dark:border-rose-700 dark:text-rose-400 dark:hover:bg-rose-950"
          >
            Wipe artifacts
          </button>
          <button
            type="button"
            onClick={() => setShowNew(true)}
            className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 dark:bg-slate-100 dark:text-slate-900"
          >
            New collection
          </button>
        </div>
      </header>

      <SchemaDiagnosticsBanner diagnostics={data.schemaDiagnostics ?? []} />

      <nav
        aria-label="Project sections"
        className="flex items-center gap-2 border-b border-slate-200 dark:border-slate-700"
      >
        <TopTabButton
          active={tab === "collections"}
          onClick={() => setTab("collections")}
          data-testid="project-tab-collections"
        >
          Collections
        </TopTabButton>
        <TopTabButton
          active={tab === "suggestions"}
          onClick={() => setTab("suggestions")}
          data-testid="project-tab-suggestions"
        >
          Suggested links
        </TopTabButton>
      </nav>

      {tab === "collections" ? (
        <div className="space-y-3">
          {data.collections.length > 0 ? (
            data.collections.map((collection) => (
              <CollectionCard
                key={collection.prefix}
                projectSlug={data.slug}
                collection={collection}
              />
            ))
          ) : (
            <EmptyCollectionsState projectSlug={data.slug} />
          )}
        </div>
      ) : (
        <SuggestedLinksTab projectSlug={data.slug} />
      )}

      {showNew ? (
        <NewCollectionDialog
          projectSlug={data.slug}
          onClose={() => setShowNew(false)}
        />
      ) : null}

      {showImport ? (
        <DoorstopImportDialog
          projectSlug={data.slug}
          onClose={() => setShowImport(false)}
        />
      ) : null}

      {showMigrate ? (
        <MigrateSchemaDialog
          projectSlug={data.slug}
          onClose={() => setShowMigrate(false)}
        />
      ) : null}

      {showWipe ? (
        <WipeArtifactsDialog
          projectSlug={data.slug}
          onClose={() => setShowWipe(false)}
        />
      ) : null}
    </section>
  );
}

interface TopTabButtonProps {
  readonly active: boolean;
  readonly onClick: () => void;
  readonly children: React.ReactNode;
  readonly "data-testid"?: string;
}

function TopTabButton({
  active,
  onClick,
  children,
  "data-testid": testid,
}: TopTabButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={testid}
      aria-pressed={active}
      className={
        active
          ? "-mb-px border-b-2 border-slate-900 px-3 py-2 text-sm font-semibold dark:border-slate-100"
          : "-mb-px border-b-2 border-transparent px-3 py-2 text-sm text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-100"
      }
    >
      {children}
    </button>
  );
}
