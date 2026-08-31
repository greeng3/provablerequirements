import type { ProofCoverage } from "../../../api/types";

type Props = {
  coverage: ProofCoverage;
};

type Segment = { key: string; label: string; count: number; className: string };

/// The triage composition of the backlog as a single stacked bar, plus the
/// pipeline tallies (drafting / verified / stale) as tabular stats. Reads the
/// honest funnel: untriaged is its own segment, never folded into prose.
export function CoverageBar({ coverage }: Props) {
  const segments: Segment[] = [
    {
      key: "formalizable_now",
      label: "formalizable",
      count: coverage.formalizable_now,
      className: "bg-sky-500",
    },
    {
      key: "falsifiable_only",
      label: "falsifiable",
      count: coverage.falsifiable_only,
      className: "bg-amber-500",
    },
    {
      key: "stays_prose",
      label: "prose",
      count: coverage.stays_prose,
      className: "bg-slate-400",
    },
    {
      key: "untriaged",
      label: "untriaged",
      count: coverage.untriaged,
      className: "bg-slate-300 dark:bg-slate-600",
    },
  ];
  const total = coverage.discovered || 1;

  return (
    <section aria-label="Coverage" className="flex flex-col gap-4">
      <div className="flex items-baseline justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wide text-slate-500">
          Coverage
        </h2>
        <span className="text-sm text-slate-500">
          {coverage.discovered} discovered
        </span>
      </div>

      <div
        className="flex h-2.5 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800"
        role="img"
        aria-label={`Triage split: ${segments
          .filter((s) => s.count > 0)
          .map((s) => `${s.count} ${s.label}`)
          .join(", ")}`}
      >
        {segments
          .filter((s) => s.count > 0)
          .map((s) => (
            <div
              key={s.key}
              className={s.className}
              style={{ width: `${(s.count / total) * 100}%` }}
            />
          ))}
      </div>

      <dl className="grid grid-cols-3 gap-3 sm:grid-cols-7">
        {segments.map((s) => (
          <Stat key={s.key} label={s.label} value={s.count} />
        ))}
        <Stat label="drafting" value={coverage.drafting} />
        <Stat label="verified" value={coverage.verified} />
        {/* The living loop's re-verify worklist — amber only when work is owed. */}
        <Stat label="stale" value={coverage.stale} warn={coverage.stale > 0} />
      </dl>
    </section>
  );
}

function Stat({
  label,
  value,
  warn = false,
}: {
  label: string;
  value: number;
  warn?: boolean;
}) {
  return (
    <div
      className={`rounded-lg border px-3 py-2 ${
        warn
          ? "border-amber-300 bg-amber-50 dark:border-amber-800 dark:bg-amber-950/30"
          : "border-slate-200 bg-slate-50 dark:border-slate-800 dark:bg-slate-900"
      }`}
    >
      <dd
        className={`text-lg font-semibold tabular-nums ${
          warn ? "text-amber-700 dark:text-amber-300" : ""
        }`}
      >
        {value}
      </dd>
      <dt className="text-xs text-slate-500">{label}</dt>
    </div>
  );
}
