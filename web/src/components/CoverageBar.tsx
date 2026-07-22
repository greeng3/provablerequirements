import type { Coverage } from "../types";

type Props = {
  coverage: Coverage;
};

type Segment = { key: string; label: string; count: number; className: string };

/**
 * The A2 triage composition of the backlog as a single stacked bar, plus the
 * pipeline tallies (drafting / formalized / verified) as tabular stats. Reads
 * the honest funnel: untriaged is its own segment, never folded into prose.
 */
export function CoverageBar({ coverage }: Props) {
  const segments: Segment[] = [
    { key: "formalizable_now", label: "formalizable", count: coverage.formalizable_now, className: "bg-info" },
    { key: "falsifiable_only", label: "falsifiable", count: coverage.falsifiable_only, className: "bg-warn" },
    { key: "stays_prose", label: "prose", count: coverage.stays_prose, className: "bg-muted" },
    { key: "untriaged", label: "untriaged", count: coverage.untriaged, className: "bg-border" },
  ];
  const total = coverage.discovered || 1;

  return (
    <section aria-label="Coverage" className="flex flex-col gap-4">
      <div className="flex items-baseline justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wide text-muted">
          Coverage
        </h2>
        <span className="text-sm text-muted">
          {coverage.discovered} discovered
        </span>
      </div>

      <div
        className="flex h-2.5 overflow-hidden rounded-full bg-surface-2"
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

      <dl className="grid grid-cols-3 gap-3 sm:grid-cols-6">
        {segments.map((s) => (
          <Stat key={s.key} label={s.label} value={s.count} />
        ))}
        <Stat label="drafting" value={coverage.drafting} />
        <Stat label="verified" value={coverage.verified} />
      </dl>
    </section>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg border border-border bg-surface-2 px-3 py-2">
      <dd className="text-lg font-semibold tabular-nums">{value}</dd>
      <dt className="text-xs text-muted">{label}</dt>
    </div>
  );
}
