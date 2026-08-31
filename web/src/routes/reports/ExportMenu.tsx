import type { ReportKind, ReportScopeParam } from "../../api/types";

interface Props {
  readonly kind: ReportKind;
  readonly scope: ReportScopeParam;
  readonly includeInactive: boolean;
  /// Per-kind extras threaded into the export URL the same way
  /// the live `useReport` hook threads them — coverage-matrix
  /// covering types, impact-analysis seed + direction, etc.
  readonly extra?: Record<string, string | undefined>;
}

/// Three `<a href>` download links — JSON, CSV, HTML — wired
/// against the Phase 6b export endpoint. Links freeze the
/// current UI state (scope, include-inactive, per-kind extras)
/// at render time, so clicking produces a snapshot of what's
/// on-screen rather than following later config drift.
///
/// The Cycles report's CSV link is visually disabled with a
/// tooltip pointing at JSON / HTML — the backend would return
/// 406, and we preempt the broken click.
export function ExportMenu({ kind, scope, includeInactive, extra }: Props) {
  const params = buildSearchParams(scope, includeInactive, extra);
  const base = (ext: string) => {
    const qs = params.toString();
    return `/api/reports/${encodeURIComponent(kind)}/export/${ext}${
      qs ? `?${qs}` : ""
    }`;
  };
  const csvDisabled = kind === "cycles";
  return (
    <nav
      aria-label="Export report"
      className="flex flex-wrap items-center gap-2 text-xs"
    >
      <span className="text-xs uppercase tracking-wide text-slate-500">
        Export
      </span>
      <ExportLink href={base("json")} label="JSON" />
      {csvDisabled ? (
        <span
          role="button"
          aria-disabled="true"
          title="Cycles has no natural flat CSV encoding — use JSON or HTML instead."
          className="cursor-not-allowed rounded border border-slate-200 px-2 py-1 text-slate-400 dark:border-slate-700 dark:text-slate-500"
        >
          CSV
        </span>
      ) : (
        <ExportLink href={base("csv")} label="CSV" />
      )}
      <ExportLink href={base("html")} label="HTML" />
    </nav>
  );
}

function ExportLink({ href, label }: { href: string; label: string }) {
  return (
    <a
      href={href}
      download
      className="rounded border border-slate-300 px-2 py-1 hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
    >
      {label}
    </a>
  );
}

function buildSearchParams(
  scope: ReportScopeParam,
  includeInactive: boolean,
  extra?: Record<string, string | undefined>,
): URLSearchParams {
  const params = new URLSearchParams();
  if (scope && scope !== "system") params.set("scope", scope);
  if (includeInactive) params.set("includeInactive", "true");
  if (extra) {
    for (const [key, value] of Object.entries(extra)) {
      if (value !== undefined && value !== "") params.set(key, value);
    }
  }
  return params;
}
