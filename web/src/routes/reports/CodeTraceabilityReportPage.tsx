import { useState } from "react";
import { Link } from "react-router-dom";

import { useReport } from "../../api/queries";
import type {
  CodeTraceabilityEntry,
  CodeTraceabilityOrphan,
  CodeTraceabilityReportPayload,
  ReportScopeParam,
} from "../../api/types";

import { ReportHeader } from "./ReportHeader";

/// Phase 9b code-traceability report. Per-artifact listing of
/// in-code tag locations grouped by Phase 9a canonical verb,
/// plus a separate orphan-tag list and an "uncovered"
/// highlight for artifacts whose collection / per-artifact
/// `expectsCodeTrace` resolves true but whose tag lookup is
/// empty.
export function CodeTraceabilityReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const report = useReport("code-traceability", scope, includeInactive);

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="code-traceability"
        title="Code traceability"
        description="Artifacts and the source / test file locations referencing them via in-code tags. Covers the REPORT-codeTraceability surface: locations grouped by verb, orphan tags flagged separately, artifacts expecting code trace but uncovered highlighted as gaps."
        scope={scope}
        includeInactive={includeInactive}
        onScopeChange={setScope}
        onIncludeInactiveChange={setIncludeInactive}
        onResetToDefaults={() => {
          setScope("system");
          setIncludeInactive(false);
        }}
      />

      {report.isLoading ? (
        <p className="text-sm text-slate-500">Loading…</p>
      ) : report.isError || !report.data ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load report: {String(report.error ?? "unknown")}
        </p>
      ) : report.data.kind !== "code-traceability" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body report={report.data} />
      )}
    </section>
  );
}

function Body({ report }: { report: CodeTraceabilityReportPayload }) {
  return (
    <div className="space-y-4">
      <Summary report={report} />
      {report.entries.length === 0 ? (
        <p className="text-sm text-slate-500">
          No artifacts in scope. Try broadening the scope selector.
        </p>
      ) : (
        <ul className="space-y-2">
          {report.entries.map((entry) => (
            <EntryRow key={entry.artifact.uuid} entry={entry} />
          ))}
        </ul>
      )}
      <OrphanTagList orphans={report.orphanTags} />
    </div>
  );
}

function Summary({ report }: { report: CodeTraceabilityReportPayload }) {
  return (
    <p
      data-testid="code-trace-summary"
      className="text-sm text-slate-600 dark:text-slate-400"
    >
      {report.totalArtifacts} artifact
      {report.totalArtifacts === 1 ? "" : "s"} in scope ·{" "}
      <span className={report.uncoveredCount > 0 ? "text-rose-700" : ""}>
        {report.uncoveredCount} uncovered
      </span>{" "}
      ·{" "}
      <span className={report.orphanTagCount > 0 ? "text-amber-700" : ""}>
        {report.orphanTagCount} orphan tag
        {report.orphanTagCount === 1 ? "" : "s"}
      </span>
    </p>
  );
}

function EntryRow({ entry }: { entry: CodeTraceabilityEntry }) {
  const [open, setOpen] = useState(false);
  const verbs = Object.keys(entry.locationsByVerb).sort();
  const totalLocations = verbs.reduce(
    (n, v) => n + entry.locationsByVerb[v].length,
    0,
  );
  const statusBadge = entry.hasGap ? (
    <span className="rounded bg-rose-100 px-1.5 py-0.5 text-xs text-rose-800 dark:bg-rose-900/40 dark:text-rose-200">
      gap
    </span>
  ) : verbs.length === 0 ? (
    <span className="rounded bg-slate-100 px-1.5 py-0.5 text-xs text-slate-700 dark:bg-slate-800 dark:text-slate-200">
      no tags
    </span>
  ) : (
    <span className="rounded bg-emerald-100 px-1.5 py-0.5 text-xs text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200">
      covered · {totalLocations}
    </span>
  );

  return (
    <li
      data-testid={`code-trace-entry-${entry.artifact.artifactName}`}
      className={`rounded border p-3 ${
        entry.hasGap
          ? "border-rose-300 bg-rose-50 dark:border-rose-800 dark:bg-rose-900/20"
          : "border-slate-200 dark:border-slate-800"
      }`}
    >
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <div>
          <Link
            to={`/projects/${entry.artifact.projectSlug}/collections/${entry.artifact.collectionPrefix}/artifacts/${entry.artifact.artifactName}`}
            className="font-mono text-xs text-sky-700 underline dark:text-sky-300"
          >
            {entry.artifact.projectSlug}/{entry.artifact.collectionPrefix}/
            {entry.artifact.artifactName}
          </Link>
          <span className="ml-2 text-sm text-slate-600 dark:text-slate-400">
            {entry.artifact.title}
          </span>
        </div>
        {statusBadge}
      </div>
      {verbs.length > 0 ? (
        <details
          className="mt-2"
          open={open}
          onToggle={(e) => setOpen((e.target as HTMLDetailsElement).open)}
        >
          <summary className="cursor-pointer text-xs text-slate-500 hover:text-slate-700 dark:hover:text-slate-300">
            {open ? "hide" : "show"} {verbs.length} verb
            {verbs.length === 1 ? "" : "s"} · {totalLocations} location
            {totalLocations === 1 ? "" : "s"}
          </summary>
          <div className="mt-2 space-y-2">
            {verbs.map((verb) => (
              <div
                key={verb}
                data-testid={`code-trace-verb-${entry.artifact.artifactName}-${verb}`}
              >
                <p className="text-xs font-semibold uppercase tracking-wide text-slate-500">
                  {verb}
                </p>
                <ul className="ml-3 space-y-0.5 text-xs">
                  {entry.locationsByVerb[verb].map((loc, idx) => (
                    <li key={`${loc.file}-${loc.line}-${idx}`}>
                      <code className="font-mono text-slate-700 dark:text-slate-200">
                        {loc.file}:{loc.line}
                      </code>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </details>
      ) : !entry.expectsCodeTrace ? (
        <p className="mt-2 text-xs text-slate-500">
          Collection / artifact setting marks this as not expecting code trace.
        </p>
      ) : null}
    </li>
  );
}

function OrphanTagList({ orphans }: { orphans: CodeTraceabilityOrphan[] }) {
  if (orphans.length === 0) {
    return null;
  }
  return (
    <section
      aria-labelledby="code-trace-orphans-heading"
      className="rounded border border-amber-300 bg-amber-50 p-3 dark:border-amber-700 dark:bg-amber-900/20"
    >
      <h2
        id="code-trace-orphans-heading"
        className="mb-2 text-sm font-semibold text-amber-900 dark:text-amber-100"
      >
        Orphan tags ({orphans.length})
      </h2>
      <p className="mb-2 text-xs text-amber-800 dark:text-amber-200">
        In-code tags whose `(prefix, name)` pair didn't resolve to any mounted
        artifact. Typically the target was renamed, or the tag has a typo.
      </p>
      <ul className="space-y-1 text-xs">
        {orphans.map((o, idx) => (
          <li key={`${o.file}-${o.line}-${idx}`}>
            <span className="mr-2 font-mono text-slate-500">{o.verb}</span>
            <code className="mr-2 rounded bg-white px-1.5 py-0.5 font-mono text-slate-800 dark:bg-slate-800 dark:text-slate-200">
              {o.rawId}
            </code>
            <span className="font-mono text-slate-700 dark:text-slate-300">
              {o.file}:{o.line}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
