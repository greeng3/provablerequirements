import { useEffect, useState, type FormEvent } from "react";

import {
  useAnalyzeLinkSuggestions,
  useDoorstopImport,
  useDoorstopPreview,
  useLlmProviders,
} from "../../api/queries";
import { ApiError } from "../../api/client";
import type {
  DoorstopImportConflictBody,
  DoorstopImportPlan,
  DoorstopImportReport,
} from "../../api/types";
import { DoorstopRenameWizard } from "./DoorstopRenameWizard";

interface Props {
  readonly projectSlug: string;
  readonly onClose: () => void;
}

type Stage =
  | { kind: "source" }
  | { kind: "preview"; plan: DoorstopImportPlan }
  | { kind: "done"; report: DoorstopImportReport };

/// Three-panel wizard launched from the project detail page.
/// Source-path input → preview (with plan summary + collision
/// warnings) → Import button → final report.
export function DoorstopImportDialog({ projectSlug, onClose }: Props) {
  const [source, setSource] = useState("");
  const [stage, setStage] = useState<Stage>({ kind: "source" });

  const preview = useDoorstopPreview(projectSlug);
  const importRun = useDoorstopImport(projectSlug);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const runPreview = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    preview.mutate(
      { source },
      {
        onSuccess: (plan) => setStage({ kind: "preview", plan }),
      },
    );
  };

  const runImport = () => {
    importRun.mutate(
      { source },
      {
        onSuccess: (report) => setStage({ kind: "done", report }),
      },
    );
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="doorstop-import-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="max-h-[90vh] w-full max-w-2xl overflow-auto rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="doorstop-import-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Import from doorstop
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          One-way import: doorstop source files are parsed, planned, and written
          as new ReqForge collections. The originals are never modified.
        </p>

        {stage.kind === "source" ? (
          <SourcePanel
            source={source}
            onSourceChange={setSource}
            onSubmit={runPreview}
            onCancel={onClose}
            isLoading={preview.isPending}
            error={preview.error}
          />
        ) : stage.kind === "preview" ? (
          <PreviewPanel
            plan={stage.plan}
            onBack={() => setStage({ kind: "source" })}
            onImport={runImport}
            isImporting={importRun.isPending}
            importError={importRun.error}
          />
        ) : (
          <ReportPanel
            projectSlug={projectSlug}
            report={stage.report}
            onClose={onClose}
          />
        )}
      </div>
    </div>
  );
}

interface SourcePanelProps {
  readonly source: string;
  readonly onSourceChange: (v: string) => void;
  readonly onSubmit: (e: FormEvent<HTMLFormElement>) => void;
  readonly onCancel: () => void;
  readonly isLoading: boolean;
  readonly error: Error | null;
}

function SourcePanel({
  source,
  onSourceChange,
  onSubmit,
  onCancel,
  isLoading,
  error,
}: SourcePanelProps) {
  return (
    <form onSubmit={onSubmit} className="mt-4 space-y-3">
      <label className="block text-sm">
        <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
          Source path
        </span>
        <input
          data-testid="doorstop-source-input"
          value={source}
          onChange={(e) => onSourceChange(e.target.value)}
          placeholder="e.g. requirements-doorstop (empty = project root)"
          aria-label="Doorstop source path"
          className="w-full rounded border border-slate-300 px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
        />
        <span className="mt-1 block text-xs text-slate-500">
          Project-root-relative path to the directory to scan for{" "}
          <code className="font-mono">.doorstop.yml</code> markers.
        </span>
      </label>
      {error ? (
        <p className="text-sm text-rose-600" role="alert">
          {String(error)}
        </p>
      ) : null}
      <div className="flex justify-end gap-2 pt-2">
        <button
          type="button"
          onClick={onCancel}
          className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={isLoading}
          data-testid="doorstop-preview-button"
          className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
        >
          {isLoading ? "Previewing…" : "Preview"}
        </button>
      </div>
    </form>
  );
}

interface PreviewPanelProps {
  readonly plan: DoorstopImportPlan;
  readonly onBack: () => void;
  readonly onImport: () => void;
  readonly isImporting: boolean;
  readonly importError: Error | null;
}

function PreviewPanel({
  plan,
  onBack,
  onImport,
  isImporting,
  importError,
}: PreviewPanelProps) {
  const totalArtifacts = plan.collections.reduce(
    (n, c) => n + c.artifacts.length,
    0,
  );
  const hasCollisions = plan.prefixCollisions.length > 0;
  const conflictBody = extractConflictBody(importError);

  return (
    <div className="mt-4 space-y-4">
      <div
        data-testid="doorstop-plan-summary"
        className="rounded border border-slate-200 p-3 text-sm dark:border-slate-700"
      >
        <p>
          <strong>{plan.collections.length}</strong> collection
          {plan.collections.length === 1 ? "" : "s"} ·{" "}
          <strong>{totalArtifacts}</strong> artifact
          {totalArtifacts === 1 ? "" : "s"} ·{" "}
          <strong>{plan.unresolvedLinks.length}</strong> unresolved link
          {plan.unresolvedLinks.length === 1 ? "" : "s"}
        </p>
      </div>

      {hasCollisions ? (
        <div
          role="alert"
          data-testid="doorstop-collision-banner"
          className="rounded border border-rose-300 bg-rose-50 p-3 text-sm text-rose-900 dark:border-rose-700 dark:bg-rose-900/30 dark:text-rose-100"
        >
          <p className="font-semibold">Prefix collision — import blocked.</p>
          <p className="mt-1">
            Resolve by renaming one of the prefixes or removing the conflicting
            ReqForge collection, then re-preview.
          </p>
          <ul className="mt-2 list-disc pl-5 font-mono text-xs">
            {plan.prefixCollisions.map((c) => (
              <li key={c.prefix}>
                {c.prefix}: existing collection directory{" "}
                <code>{c.existingCollectionDirectory}</code>
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      <ul className="space-y-2">
        {plan.collections.map((c) => (
          <li
            key={c.prefix}
            data-testid={`doorstop-plan-collection-${c.prefix}`}
            className="rounded border border-slate-200 p-3 text-sm dark:border-slate-700"
          >
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <div>
                <span className="font-mono font-semibold">{c.prefix}</span>{" "}
                <span className="text-slate-600 dark:text-slate-400">
                  {c.name}
                </span>
              </div>
              <span className="text-xs text-slate-500">
                {c.artifacts.length} artifact
                {c.artifacts.length === 1 ? "" : "s"}
              </span>
            </div>
            {c.emptyWarning ? (
              <p className="mt-1 text-xs text-amber-700 dark:text-amber-300">
                {c.emptyWarning}
              </p>
            ) : null}
          </li>
        ))}
      </ul>

      {plan.unresolvedLinks.length > 0 ? (
        <details className="rounded border border-amber-300 bg-amber-50 p-3 text-xs dark:border-amber-700 dark:bg-amber-900/30">
          <summary className="cursor-pointer font-semibold">
            {plan.unresolvedLinks.length} unresolved link
            {plan.unresolvedLinks.length === 1 ? "" : "s"}
          </summary>
          <ul className="mt-2 space-y-1 font-mono">
            {plan.unresolvedLinks.map((u, idx) => (
              <li key={idx}>
                {u.sourceUid} → <code>{u.targetUid}</code> (not imported)
              </li>
            ))}
          </ul>
        </details>
      ) : null}

      {plan.warnings.length > 0 ? (
        <details className="rounded border border-slate-300 bg-slate-50 p-3 text-xs dark:border-slate-600 dark:bg-slate-800">
          <summary className="cursor-pointer font-semibold">
            {plan.warnings.length} warning
            {plan.warnings.length === 1 ? "" : "s"}
          </summary>
          <ul className="mt-2 list-disc space-y-1 pl-5">
            {plan.warnings.map((w, idx) => (
              <li key={idx}>{w}</li>
            ))}
          </ul>
        </details>
      ) : null}

      {importError ? (
        <div
          role="alert"
          className="rounded border border-rose-300 bg-rose-50 p-3 text-sm text-rose-900 dark:border-rose-700 dark:bg-rose-900/30 dark:text-rose-100"
        >
          {conflictBody ? (
            <>
              <p className="font-semibold">
                {conflictBody.error} — import refused.
              </p>
              <ul className="mt-1 list-disc pl-5 font-mono text-xs">
                {conflictBody.collisions.map((c) => (
                  <li key={c.prefix}>
                    {c.prefix} collides with{" "}
                    <code>{c.existingCollectionDirectory}</code>
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <p>{String(importError)}</p>
          )}
        </div>
      ) : null}

      <div className="flex justify-between gap-2 pt-2">
        <button
          type="button"
          onClick={onBack}
          className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          Back
        </button>
        <button
          type="button"
          onClick={onImport}
          disabled={hasCollisions || isImporting}
          data-testid="doorstop-import-button"
          className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
        >
          {isImporting ? "Importing…" : "Import"}
        </button>
      </div>
    </div>
  );
}

interface ReportPanelProps {
  readonly projectSlug: string;
  readonly report: DoorstopImportReport;
  readonly onClose: () => void;
}

function ReportPanel({ projectSlug, report, onClose }: ReportPanelProps) {
  const exportBase = `/api/projects/${encodeURIComponent(projectSlug)}/doorstop/report/export`;
  const llmProviders = useLlmProviders();
  const hasLlm = (llmProviders.data?.providers?.length ?? 0) > 0;
  const [showRenameWizard, setShowRenameWizard] = useState(false);
  const collectionPrefixes = report.collections.map((c) => c.prefix);
  const analyze = useAnalyzeLinkSuggestions(projectSlug);
  const onAnalyze = () => {
    analyze.mutate(undefined, {
      onSettled: onClose,
    });
  };
  return (
    <div className="mt-4 space-y-4" data-testid="doorstop-report-panel">
      <div className="rounded border border-emerald-300 bg-emerald-50 p-3 text-sm text-emerald-900 dark:border-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-100">
        Import complete. {report.totals.artifactsImported} artifact
        {report.totals.artifactsImported === 1 ? "" : "s"} written across{" "}
        {report.totals.collectionsCreated} collection
        {report.totals.collectionsCreated === 1 ? "" : "s"}.
      </div>

      <dl className="grid grid-cols-2 gap-2 text-sm">
        <ReportStat
          label="Artifacts imported"
          value={report.totals.artifactsImported}
        />
        <ReportStat
          label="Collections created"
          value={report.totals.collectionsCreated}
        />
        <ReportStat label="URL artifacts" value={report.totals.urlArtifacts} />
        <ReportStat label="cites links" value={report.totals.citesLinks} />
        <ReportStat label="Legacy refs" value={report.totals.legacyRefs} />
        <ReportStat
          label="Synthetic reviews"
          value={report.totals.syntheticReviewEntries}
        />
        <ReportStat
          label="Legacy preserved"
          value={report.totals.legacyPreservedFields}
        />
        <ReportStat
          label="Unresolved links"
          value={report.totals.unresolvedLinkCount}
        />
      </dl>

      <nav
        aria-label="Export report"
        className="flex flex-wrap items-center gap-2 text-xs"
      >
        <span className="text-xs uppercase tracking-wide text-slate-500">
          Download
        </span>
        <a
          href={`${exportBase}/json`}
          download
          className="rounded border border-slate-300 px-2 py-1 hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          JSON
        </a>
        <a
          href={`${exportBase}/csv`}
          download
          className="rounded border border-slate-300 px-2 py-1 hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          CSV
        </a>
        <a
          href={`${exportBase}/html`}
          download
          className="rounded border border-slate-300 px-2 py-1 hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          HTML
        </a>
      </nav>

      <div className="flex flex-wrap justify-end gap-2 pt-2">
        {hasLlm && collectionPrefixes.length > 0 ? (
          <button
            type="button"
            onClick={() => setShowRenameWizard(true)}
            data-testid="doorstop-suggest-names-button"
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Suggest names
          </button>
        ) : null}
        {hasLlm && report.totals.artifactsImported > 0 ? (
          <button
            type="button"
            onClick={onAnalyze}
            disabled={analyze.isPending}
            data-testid="doorstop-analyze-links-button"
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 disabled:opacity-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            {analyze.isPending ? "Analyzing for links…" : "Analyze for links"}
          </button>
        ) : null}
        <button
          type="button"
          onClick={onClose}
          disabled={analyze.isPending}
          className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
        >
          Done
        </button>
      </div>
      {showRenameWizard ? (
        <DoorstopRenameWizard
          projectSlug={projectSlug}
          collectionPrefixes={collectionPrefixes}
          onClose={() => setShowRenameWizard(false)}
        />
      ) : null}
    </div>
  );
}

interface StatProps {
  readonly label: string;
  readonly value: number;
}

function ReportStat({ label, value }: StatProps) {
  return (
    <div className="rounded border border-slate-200 p-2 dark:border-slate-700">
      <dt className="text-xs uppercase tracking-wide text-slate-500">
        {label}
      </dt>
      <dd className="text-lg font-semibold">{value}</dd>
    </div>
  );
}

function extractConflictBody(
  error: Error | null,
): DoorstopImportConflictBody | null {
  if (!(error instanceof ApiError) || error.status !== 409) return null;
  const body = error.body as DoorstopImportConflictBody | undefined;
  if (!body || !Array.isArray(body.collisions)) return null;
  return body;
}
