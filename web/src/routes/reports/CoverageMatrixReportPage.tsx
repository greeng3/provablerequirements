import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useLinkTypes, useReport } from "../../api/queries";
import type {
  CoverageMatrixReportPayload,
  CoverageParentEntry,
  ReportScopeParam,
} from "../../api/types";

import { ReportHeader } from "./ReportHeader";

const DEFAULT_COVERING: string[] = ["satisfies", "verifies"];

export function CoverageMatrixReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const [coveringTypes, setCoveringTypes] =
    useState<string[]>(DEFAULT_COVERING);
  const linkTypes = useLinkTypes();

  const extra = useMemo(
    () =>
      coveringTypes.length > 0
        ? { coveringLinkTypes: coveringTypes.join(",") }
        : undefined,
    [coveringTypes],
  );
  const report = useReport("coverage-matrix", scope, includeInactive, extra);

  const toggleType = (name: string) => {
    setCoveringTypes((prev) =>
      prev.includes(name) ? prev.filter((t) => t !== name) : [...prev, name],
    );
  };

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="coverage-matrix"
        title="Coverage matrix"
        description="Parent artifacts vs the children that cover them via the configured covering link types. Parents with zero covering children are flagged as gaps."
        scope={scope}
        includeInactive={includeInactive}
        onScopeChange={setScope}
        onIncludeInactiveChange={setIncludeInactive}
        onResetToDefaults={() => {
          setScope("system");
          setIncludeInactive(false);
          setCoveringTypes(DEFAULT_COVERING);
        }}
        exportExtras={extra}
      />

      <div className="rounded border border-slate-200 p-3 text-sm dark:border-slate-800">
        <p className="mb-2 text-xs uppercase tracking-wide text-slate-500">
          Covering link types
        </p>
        {linkTypes.isLoading ? (
          <p className="text-sm text-slate-500">Loading link catalog…</p>
        ) : linkTypes.isError || !linkTypes.data ? (
          <p className="text-sm text-rose-600" role="alert">
            Failed to load link catalog.
          </p>
        ) : (
          <div className="flex flex-wrap gap-2">
            {linkTypes.data.map((t) => (
              <label
                key={t.name}
                className="flex items-center gap-1 rounded border border-slate-300 px-2 py-1 text-xs dark:border-slate-600"
              >
                <input
                  type="checkbox"
                  checked={coveringTypes.includes(t.name)}
                  onChange={() => toggleType(t.name)}
                />
                <span className="font-mono">{t.name}</span>
              </label>
            ))}
          </div>
        )}
      </div>

      {report.isLoading ? (
        <p className="text-sm text-slate-500">Loading…</p>
      ) : report.isError || !report.data ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load report: {String(report.error ?? "unknown")}
        </p>
      ) : report.data.kind !== "coverage-matrix" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body report={report.data} />
      )}
    </section>
  );
}

function Body({ report }: { report: CoverageMatrixReportPayload }) {
  return (
    <div className="space-y-3">
      {report.unknownRequestedTypes.length > 0 ? (
        <p
          role="alert"
          className="rounded border border-amber-300 bg-amber-50 p-2 text-xs text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100"
        >
          Ignored unknown link type
          {report.unknownRequestedTypes.length === 1 ? "" : "s"}:{" "}
          {report.unknownRequestedTypes.map((t) => (
            <span key={t} className="font-mono">
              {t}{" "}
            </span>
          ))}
        </p>
      ) : null}
      <p className="text-sm text-slate-600 dark:text-slate-400">
        {report.totalParents} parent artifact
        {report.totalParents === 1 ? "" : "s"} in scope ·{" "}
        <span className={report.gapCount > 0 ? "text-rose-700" : ""}>
          {report.gapCount} gap{report.gapCount === 1 ? "" : "s"}
        </span>
      </p>
      {report.totalParents === 0 ? (
        <p className="text-sm text-slate-500">
          No parent artifacts in scope. Try broadening the scope.
        </p>
      ) : (
        <ul className="space-y-2">
          {report.parents.map((p) => (
            <ParentRow key={p.parent.uuid} entry={p} />
          ))}
        </ul>
      )}
    </div>
  );
}

function ParentRow({ entry }: { entry: CoverageParentEntry }) {
  const codeEvidence = entry.coveringCodeEvidence ?? [];
  const header = (
    <div className="flex items-baseline justify-between gap-2">
      <div>
        <Link
          to={`/projects/${entry.parent.projectSlug}/collections/${entry.parent.collectionPrefix}/artifacts/${entry.parent.artifactName}`}
          className="font-mono text-xs text-sky-700 underline dark:text-sky-300"
        >
          {entry.parent.projectSlug}/{entry.parent.collectionPrefix}/
          {entry.parent.artifactName}
        </Link>
        <span className="ml-2 text-sm text-slate-600 dark:text-slate-400">
          {entry.parent.title}
        </span>
      </div>
      <div className="flex items-center gap-1">
        {codeEvidence.length > 0 ? (
          <span
            data-testid={`coverage-code-badge-${entry.parent.artifactName}`}
            title="Covering evidence from in-code tags"
            className="rounded bg-sky-100 px-1.5 py-0.5 text-xs text-sky-800 dark:bg-sky-900/40 dark:text-sky-200"
          >
            +{codeEvidence.length} code
          </span>
        ) : null}
        {entry.hasGap ? (
          <span className="rounded bg-rose-100 px-1.5 py-0.5 text-xs text-rose-800 dark:bg-rose-900/40 dark:text-rose-200">
            gap
          </span>
        ) : (
          <span className="rounded bg-emerald-100 px-1.5 py-0.5 text-xs text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200">
            covered
          </span>
        )}
      </div>
    </div>
  );
  const hasChildren = entry.coveringChildren.length > 0;
  const hasCode = codeEvidence.length > 0;
  if (!hasChildren && !hasCode) {
    return (
      <li
        className={`rounded border p-3 ${
          entry.hasGap
            ? "border-rose-300 bg-rose-50 dark:border-rose-800 dark:bg-rose-900/20"
            : "border-slate-200 dark:border-slate-800"
        }`}
      >
        {header}
      </li>
    );
  }
  return (
    <li className="rounded border border-slate-200 p-3 dark:border-slate-800">
      {header}
      {hasChildren ? (
        <ul className="mt-2 space-y-0.5 pl-4 text-xs">
          {entry.coveringChildren.map((c, idx) => (
            <li key={`${c.linkType}-${c.child.uuid}-${idx}`}>
              <span className="mr-2 font-mono text-slate-500">
                {c.linkType}
              </span>
              <Link
                to={`/projects/${c.child.projectSlug}/collections/${c.child.collectionPrefix}/artifacts/${c.child.artifactName}`}
                className="font-mono text-sky-700 underline dark:text-sky-300"
              >
                {c.child.projectSlug}/{c.child.collectionPrefix}/
                {c.child.artifactName}
              </Link>
              <span className="ml-2 text-slate-600 dark:text-slate-400">
                {c.child.title}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
      {hasCode ? (
        <ul
          data-testid={`coverage-code-list-${entry.parent.artifactName}`}
          className="mt-2 space-y-0.5 pl-4 text-xs"
        >
          {codeEvidence.map((ev, idx) => (
            <li key={`${ev.file}-${ev.line}-${idx}`}>
              <span className="mr-2 font-mono text-slate-500">{ev.verb}</span>
              <code className="font-mono text-slate-700 dark:text-slate-200">
                {ev.file}:{ev.line}
              </code>
            </li>
          ))}
        </ul>
      ) : null}
    </li>
  );
}
