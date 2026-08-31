import { useState } from "react";

import { useReport } from "../../api/queries";
import type {
  FilesystemOrphansReportPayload,
  OrphanBinaryEntry,
  OrphanSidecarEntry,
  ReportScopeParam,
} from "../../api/types";

import { AdoptOrphanDialog } from "./AdoptOrphanDialog";
import { ReportHeader } from "./ReportHeader";

export function FilesystemOrphansReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const [adoptCandidate, setAdoptCandidate] =
    useState<OrphanBinaryEntry | null>(null);
  const report = useReport("filesystem-orphans", scope, includeInactive);

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="filesystem-orphans"
        title="Filesystem orphans"
        description="On-disk blob / sidecar pairing mismatches. Binary-without-sidecar rows open the Adopt wizard; sidecar-without-binary rows need operator action on the file tree."
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
      ) : report.data.kind !== "filesystem-orphans" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body
          report={report.data}
          onAdopt={(orphan) => setAdoptCandidate(orphan)}
        />
      )}

      {adoptCandidate ? (
        <AdoptOrphanDialog
          orphan={adoptCandidate}
          onClose={() => setAdoptCandidate(null)}
        />
      ) : null}
    </section>
  );
}

function Body({
  report,
  onAdopt,
}: {
  report: FilesystemOrphansReportPayload;
  onAdopt: (entry: OrphanBinaryEntry) => void;
}) {
  if (report.missingSidecar.length === 0 && report.missingBinary.length === 0) {
    return (
      <p className="text-sm text-slate-500">
        No filesystem-orphans in scope. 🎉
      </p>
    );
  }
  return (
    <div className="space-y-4">
      <MissingSidecarTable entries={report.missingSidecar} onAdopt={onAdopt} />
      <MissingBinaryTable entries={report.missingBinary} />
    </div>
  );
}

function MissingSidecarTable({
  entries,
  onAdopt,
}: {
  entries: OrphanBinaryEntry[];
  onAdopt: (entry: OrphanBinaryEntry) => void;
}) {
  if (entries.length === 0) {
    return null;
  }
  return (
    <section>
      <h2 className="mb-2 text-sm font-semibold tracking-wide text-slate-700 dark:text-slate-300">
        Binaries without a sidecar ({entries.length})
      </h2>
      <div className="overflow-auto rounded border border-slate-200 dark:border-slate-800">
        <table
          className="w-full border-collapse text-sm"
          aria-label="Binaries without sidecars"
        >
          <thead className="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500 dark:bg-slate-900">
            <tr>
              <th className="p-2">File</th>
              <th className="p-2">Collection</th>
              <th className="p-2">Media type</th>
              <th className="p-2">Size</th>
              <th className="p-2" aria-label="action"></th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e) => (
              <tr
                key={`${e.projectSlug}:${e.binaryRelativePath}`}
                className="border-t border-slate-200 dark:border-slate-800"
              >
                <td className="p-2 font-mono text-xs">
                  {e.binaryRelativePath}
                </td>
                <td className="p-2 font-mono text-xs">
                  {e.projectSlug}/{e.collectionPrefix}
                </td>
                <td className="p-2 font-mono text-xs">{e.mediaType}</td>
                <td className="p-2 text-xs">{formatBytes(e.byteSize)}</td>
                <td className="p-2 text-right">
                  <button
                    type="button"
                    onClick={() => onAdopt(e)}
                    className="rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
                  >
                    Adopt as artifact
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function MissingBinaryTable({ entries }: { entries: OrphanSidecarEntry[] }) {
  if (entries.length === 0) {
    return null;
  }
  return (
    <section>
      <h2 className="mb-2 text-sm font-semibold tracking-wide text-slate-700 dark:text-slate-300">
        Sidecars whose blob is missing ({entries.length})
      </h2>
      <p className="mb-2 text-xs text-slate-500">
        ReqForge will never delete these automatically. Restore the binary via
        git / filesystem, or delete the sidecar manually.
      </p>
      <div className="overflow-auto rounded border border-slate-200 dark:border-slate-800">
        <table
          className="w-full border-collapse text-sm"
          aria-label="Sidecars without binaries"
        >
          <thead className="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500 dark:bg-slate-900">
            <tr>
              <th className="p-2">Sidecar</th>
              <th className="p-2">Collection</th>
              <th className="p-2">Declared blob path</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e) => (
              <tr
                key={`${e.projectSlug}:${e.sidecarFilename}`}
                className="border-t border-slate-200 dark:border-slate-800"
              >
                <td className="p-2 font-mono text-xs">{e.sidecarFilename}</td>
                <td className="p-2 font-mono text-xs">
                  {e.projectSlug}/{e.collectionPrefix}
                </td>
                <td className="p-2 font-mono text-xs">{e.declaredBlobPath}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unitIdx = 0;
  while (value >= 1024 && unitIdx < units.length - 1) {
    value /= 1024;
    unitIdx += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unitIdx]}`;
}
