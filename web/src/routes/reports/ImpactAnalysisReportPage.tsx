import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useArtifactSearch, useReport } from "../../api/queries";
import type {
  ImpactAnalysisReportPayload,
  ImpactDirection,
  ReportScopeParam,
} from "../../api/types";

import { ReportHeader } from "./ReportHeader";

export function ImpactAnalysisReportPage() {
  const [scope, setScope] = useState<ReportScopeParam>("system");
  const [includeInactive, setIncludeInactive] = useState(false);
  const [seedUuid, setSeedUuid] = useState<string | undefined>(undefined);
  const [direction, setDirection] = useState<ImpactDirection>("dependents");

  const extra = useMemo(
    () => ({
      seed: seedUuid,
      direction,
    }),
    [seedUuid, direction],
  );
  const report = useReport("impact-analysis", scope, includeInactive, extra);

  return (
    <section className="space-y-4">
      <ReportHeader
        kind="impact-analysis"
        title="Impact analysis"
        description="Transitive impact from a seed artifact. 'Dependents' walks incoming links; 'Dependencies' walks outgoing."
        scope={scope}
        includeInactive={includeInactive}
        onScopeChange={setScope}
        onIncludeInactiveChange={setIncludeInactive}
        onResetToDefaults={() => {
          setScope("system");
          setIncludeInactive(false);
          setSeedUuid(undefined);
          setDirection("dependents");
        }}
        exportExtras={extra}
      />

      <div className="flex flex-wrap items-center gap-4 rounded border border-slate-200 p-3 text-sm dark:border-slate-800">
        <SeedPicker
          seedUuid={seedUuid}
          currentSeedLabel={
            report.data?.kind === "impact-analysis"
              ? report.data.seed
                ? `${report.data.seed.projectSlug}/${report.data.seed.collectionPrefix}/${report.data.seed.artifactName}`
                : undefined
              : undefined
          }
          onSelect={setSeedUuid}
        />
        <fieldset className="flex items-center gap-3 text-xs">
          <legend className="sr-only">Direction</legend>
          <label className="flex items-center gap-1">
            <input
              type="radio"
              checked={direction === "dependents"}
              onChange={() => setDirection("dependents")}
            />
            <span>Dependents</span>
          </label>
          <label className="flex items-center gap-1">
            <input
              type="radio"
              checked={direction === "dependencies"}
              onChange={() => setDirection("dependencies")}
            />
            <span>Dependencies</span>
          </label>
        </fieldset>
      </div>

      {report.isLoading ? (
        <p className="text-sm text-slate-500">Loading…</p>
      ) : report.isError || !report.data ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load report: {String(report.error ?? "unknown")}
        </p>
      ) : report.data.kind !== "impact-analysis" ? (
        <p className="text-sm text-rose-600" role="alert">
          Unexpected report kind: {report.data.kind}
        </p>
      ) : (
        <Body report={report.data} />
      )}
    </section>
  );
}

function SeedPicker({
  seedUuid,
  currentSeedLabel,
  onSelect,
}: {
  seedUuid: string | undefined;
  currentSeedLabel?: string;
  onSelect: (uuid: string | undefined) => void;
}) {
  const [q, setQ] = useState("");
  const results = useArtifactSearch(q);
  return (
    <div className="relative min-w-[18rem] flex-1">
      <label className="block text-xs uppercase tracking-wide text-slate-500">
        Seed artifact
      </label>
      <input
        value={q}
        onChange={(e) => setQ(e.target.value)}
        placeholder={
          currentSeedLabel ?? "Search for an artifact by name or title…"
        }
        className="mt-1 w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
      />
      {q.length > 0 && results.data && results.data.length > 0 ? (
        <ul className="absolute z-10 mt-1 max-h-64 w-full overflow-auto rounded border border-slate-200 bg-white text-sm shadow-lg dark:border-slate-700 dark:bg-slate-900">
          {results.data.map((r) => (
            <li key={r.uuid}>
              <button
                type="button"
                onClick={() => {
                  onSelect(r.uuid);
                  setQ("");
                }}
                className="flex w-full flex-col px-2 py-1 text-left hover:bg-slate-100 dark:hover:bg-slate-800"
              >
                <span className="font-mono text-xs">
                  {r.projectSlug}/{r.collectionPrefix}/{r.artifactName}
                </span>
                <span className="text-slate-500">{r.title}</span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      {seedUuid ? (
        <p className="mt-1 text-xs text-slate-500">
          seed = <span className="font-mono">{seedUuid.slice(0, 8)}</span>…{" "}
          <button
            type="button"
            onClick={() => onSelect(undefined)}
            className="text-sky-700 underline hover:text-sky-900 dark:text-sky-300"
          >
            clear
          </button>
        </p>
      ) : null}
    </div>
  );
}

function Body({ report }: { report: ImpactAnalysisReportPayload }) {
  if (report.missingSeedReason) {
    return <p className="text-sm text-slate-500">{report.missingSeedReason}</p>;
  }
  if (!report.seed) {
    return null;
  }
  if (report.totalImpacted === 0) {
    return (
      <p className="text-sm text-slate-500">
        Seed has no {report.direction} in scope. 🎉
      </p>
    );
  }
  return (
    <div className="space-y-3">
      <p className="text-sm text-slate-600 dark:text-slate-400">
        {report.totalImpacted} artifact{report.totalImpacted === 1 ? "" : "s"}{" "}
        transitively{" "}
        {report.direction === "dependents" ? "depend on" : "depended on by"}{" "}
        <Link
          to={`/projects/${report.seed.projectSlug}/collections/${report.seed.collectionPrefix}/artifacts/${report.seed.artifactName}`}
          className="font-mono text-sky-700 underline dark:text-sky-300"
        >
          {report.seed.projectSlug}/{report.seed.collectionPrefix}/
          {report.seed.artifactName}
        </Link>
      </p>
      <div className="overflow-auto rounded border border-slate-200 dark:border-slate-800">
        <table
          className="w-full border-collapse text-sm"
          aria-label="Impact-analysis results"
        >
          <thead className="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500 dark:bg-slate-900">
            <tr>
              <th className="p-2">Depth</th>
              <th className="p-2">Artifact</th>
              <th className="p-2">Link types</th>
            </tr>
          </thead>
          <tbody>
            {report.impacted.map((e) => (
              <tr
                key={e.node.uuid}
                className="border-t border-slate-200 dark:border-slate-800"
              >
                <td className="p-2 font-mono text-xs">{e.depth}</td>
                <td className="p-2">
                  <Link
                    to={`/projects/${e.node.projectSlug}/collections/${e.node.collectionPrefix}/artifacts/${e.node.artifactName}`}
                    className="font-mono text-xs text-sky-700 underline dark:text-sky-300"
                  >
                    {e.node.projectSlug}/{e.node.collectionPrefix}/
                    {e.node.artifactName}
                  </Link>
                  <span className="ml-2 text-slate-500">{e.node.title}</span>
                </td>
                <td className="p-2 font-mono text-xs">
                  {e.linkTypes.join(", ")}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
