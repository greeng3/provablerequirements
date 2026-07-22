import { useState } from "react";
import { verifyRequirement } from "../api";
import type { EvidenceReport, VerdictReport, VerifyResponse } from "../types";
import type { Tone } from "../labels";
import { Badge } from "./Badge";

type Props = {
  id: string;
};

type Run =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "done"; result: VerifyResponse }
  | { kind: "error"; message: string };

// The verdict polarity tone: a hold reads calm (ok), a refutation warns, an unknown is muted —
// an unknown is honestly "no answer", never dressed up as either outcome.
const STATUS_TONE: Record<string, Tone> = {
  holds: "ok",
  fails: "warn",
  unknown: "muted",
};

function statusTone(status: string): Tone {
  return STATUS_TONE[status] ?? "muted";
}

// The honest not-yet-verifiable states carry no verdict — each names what the operator must do
// first, so the panel never shows a blank or a fabricated result.
const NOT_VERIFIABLE: Record<string, string> = {
  "no-draft": "Not formalized yet — draft a candidate PRL before verifying.",
  "not-admitted": "The draft is not admitted — admit the formalization first.",
  "no-candidate": "The admitted draft has no candidate PRL to verify.",
};

export function VerifyPanel({ id }: Props) {
  const [run, setRun] = useState<Run>({ kind: "idle" });

  const handleVerify = () => {
    setRun({ kind: "running" });
    verifyRequirement(id)
      .then((result) => setRun({ kind: "done", result }))
      .catch((err: unknown) =>
        setRun({ kind: "error", message: err instanceof Error ? err.message : String(err) }),
      );
  };

  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-center gap-3">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-muted">Verification</h3>
        <button
          type="button"
          onClick={handleVerify}
          disabled={run.kind === "running"}
          className="rounded-md border border-accent/40 bg-accent/10 px-3 py-1 text-sm font-medium text-accent hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {run.kind === "running" ? "Running engines…" : "Verify"}
        </button>
      </div>

      {run.kind === "running" && (
        <p role="status" className="text-sm text-muted">
          Running the verification engines — this can take a while.
        </p>
      )}
      {run.kind === "error" && (
        <p role="alert" className="text-sm text-warn">
          {run.message}
        </p>
      )}
      {run.kind === "done" && <Result result={run.result} />}
    </section>
  );
}

function Result({ result }: { result: VerifyResponse }) {
  if (result.state === "gate-failed") {
    return (
      <div role="status" className="flex flex-col gap-1 text-sm text-warn">
        <p>The admitted candidate no longer passes the gate — re-check it:</p>
        <ul className="ml-1 list-inside list-disc text-xs">
          {result.errors.map((e, i) => (
            <li key={i}>{e}</li>
          ))}
        </ul>
      </div>
    );
  }
  if (result.state !== "verdict") {
    return (
      <p role="status" className="text-sm text-muted">
        {NOT_VERIFIABLE[result.state]}
      </p>
    );
  }
  return <VerdictView verdict={result.verdict} stale={result.stale} />;
}

function VerdictView({ verdict, stale }: { verdict: VerdictReport; stale: boolean }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Badge label={verdict.status} tone={statusTone(verdict.status)} />
        {verdict.basis && <span className="text-xs text-muted">{verdict.basis}</span>}
        {verdict.reason && <span className="text-xs text-muted">({verdict.reason})</span>}
        {stale && <Badge label="prose moved" tone="warn" />}
      </div>

      {verdict.detail.length > 0 && (
        <ul className="ml-1 list-inside list-disc text-xs text-muted">
          {verdict.detail.map((d, i) => (
            <li key={i}>{d}</li>
          ))}
        </ul>
      )}

      {verdict.witness && (
        <div className="flex flex-col gap-1">
          <span className="text-xs font-semibold uppercase tracking-wide text-muted">
            Witness (replay to re-check)
          </span>
          <pre className="overflow-x-auto rounded-lg border border-border bg-surface-2 p-3 text-xs">
            {verdict.witness}
          </pre>
        </div>
      )}

      {verdict.evidence.length > 0 && (
        <ul className="flex flex-col gap-1.5 text-sm">
          {verdict.evidence.map((e) => (
            <EvidenceRow key={e.engine} evidence={e} />
          ))}
        </ul>
      )}

      <p className="text-[0.7rem] leading-snug text-muted">
        requirement@{verdict.provenance.requirement_revision} · subject@
        {verdict.provenance.subject_commit ?? "(not a git subject)"} · provreq@
        {verdict.provenance.tool_version}
      </p>
    </div>
  );
}

function EvidenceRow({ evidence }: { evidence: EvidenceReport }) {
  return (
    <li className="flex flex-col gap-0.5">
      <div className="flex items-center gap-2">
        <Badge label={evidence.status} tone={statusTone(evidence.status)} />
        <code className="rounded bg-surface-2 px-1.5 py-0.5 text-xs">{evidence.engine}</code>
        {evidence.basis && <span className="text-xs text-muted">{evidence.basis}</span>}
      </div>
      {evidence.detail.length > 0 && (
        <ul className="ml-2 list-inside list-disc text-xs text-muted">
          {evidence.detail.map((d, i) => (
            <li key={i}>{d}</li>
          ))}
        </ul>
      )}
    </li>
  );
}
