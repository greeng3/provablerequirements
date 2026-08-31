import { useVerifyRequirement } from "../../../api/queries";
import type {
  ProofEvidenceReport,
  ProofVerdictReport,
  ProofVerdictView,
  ProofVerifyResponse,
} from "../../../api/types";
import type { Tone } from "../labels";
import { Badge } from "./Badge";
import { BasisLabel } from "./BasisLabel";
import { VerdictBadge } from "./VerdictBadge";

type Props = {
  id: string;
  /** The last stored verdict for this item (REQ039), shown until a fresh run replaces it. */
  stored: ProofVerdictView | null;
};

// The verdict polarity tone: a hold reads calm (ok), a refutation warns, an
// unknown is muted — an unknown is honestly "no answer", never dressed up as
// either outcome.
const STATUS_TONE: Record<string, Tone> = {
  holds: "ok",
  fails: "warn",
  unknown: "muted",
};

function statusTone(status: string): Tone {
  return STATUS_TONE[status] ?? "muted";
}

// The honest not-yet-verifiable states carry no verdict — each names what the
// operator must do first, so the panel never shows a blank or a fabricated result.
const NOT_VERIFIABLE: Record<string, string> = {
  "no-draft": "Not formalized yet — draft a candidate PRL before verifying.",
  "not-admitted": "The draft is not admitted — admit the formalization first.",
  "no-candidate": "The admitted draft has no candidate PRL to verify.",
};

export function VerifyPanel({ id, stored }: Props) {
  const mutation = useVerifyRequirement();

  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-center gap-3">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500">
          Verification
        </h3>
        <button
          type="button"
          onClick={() => mutation.mutate(id)}
          disabled={mutation.isPending}
          className="rounded-md border border-sky-300 bg-sky-50 px-3 py-1 text-sm font-medium text-sky-700 hover:bg-sky-100 disabled:cursor-not-allowed disabled:opacity-60 dark:border-sky-800 dark:bg-sky-950/40 dark:text-sky-300 dark:hover:bg-sky-900/40"
        >
          {mutation.isPending
            ? "Running engines…"
            : stored
              ? "Re-verify"
              : "Verify"}
        </button>
      </div>

      {mutation.isIdle && stored && <StoredVerdict verdict={stored} />}
      {mutation.isPending && (
        <p role="status" className="text-sm text-slate-500">
          Running the verification engines — this can take a while.
        </p>
      )}
      {mutation.isError && (
        <p role="alert" className="text-sm text-amber-700 dark:text-amber-300">
          {String(mutation.error)}
        </p>
      )}
      {mutation.isSuccess && <Result result={mutation.data} />}
    </section>
  );
}

function StoredVerdict({ verdict }: { verdict: ProofVerdictView }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex flex-wrap items-center gap-2 text-sm">
        <span className="text-xs text-slate-500">Last verdict:</span>
        <VerdictBadge verdict={verdict} />
        {verdict.basis && <BasisLabel basis={verdict.basis} />}
      </div>
      {!verdict.fresh && (
        <ul className="ml-1 list-inside list-disc text-xs text-amber-700 dark:text-amber-300">
          {verdict.stale_reasons.map((r, i) => (
            <li key={i}>{r}</li>
          ))}
        </ul>
      )}
      <Grounds verdict={verdict} />
      <ProvedIn environment={verdict.environment} />
    </div>
  );
}

/// What a verdict was reached on: its own lines (for category 2a, the model it
/// was checked under), the counterexample behind a refutation, and the
/// per-engine breakdown. Shared by the stored verdict and the just-run one on
/// purpose (#218) — the same verdict must not read differently depending on
/// whether the operator happened to press the button this session.
function Grounds({
  verdict,
}: {
  verdict: Pick<ProofVerdictReport, "detail" | "witness" | "evidence">;
}) {
  // `detail` and `evidence` are never both meaningful (#220). A verdict built
  // from engines folds every engine's lines into `detail` behind a head line
  // (the CLI renders `detail` alone). A verdict with no engines carries its own
  // lines in `detail` and no evidence. So evidence, when present, already says
  // everything `detail` would, attributed to the engine that earned it.
  const showsDetail = verdict.evidence.length === 0;
  return (
    <>
      {showsDetail && verdict.detail.length > 0 && (
        <ul className="ml-1 list-inside list-disc text-xs text-slate-500">
          {verdict.detail.map((d, i) => (
            <li key={i}>{d}</li>
          ))}
        </ul>
      )}

      {verdict.witness && (
        <div className="flex flex-col gap-1">
          <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">
            Witness (replay to re-check)
          </span>
          <pre className="overflow-x-auto rounded-lg border border-slate-200 bg-slate-50 p-3 text-xs dark:border-slate-800 dark:bg-slate-900">
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
    </>
  );
}

/// Where a stored verdict was proved (REQ050). A verdict with no recorded
/// environment is called out rather than left to look like one whose environment
/// was checked and found unchanged — both are `fresh`, so silence here would let
/// the operator read a guarantee the record does not carry.
function ProvedIn({ environment }: { environment: string | null }) {
  if (environment === null) {
    return (
      <p className="ml-1 text-xs text-slate-500">
        <span className="font-medium">Environment not recorded</span> — this
        verdict predates environment recording, so where it was proved is
        unknown. Re-verify to record it.
      </p>
    );
  }
  return (
    <p className="ml-1 text-xs text-slate-500">
      <span className="font-medium">Proved in:</span> {environment}
    </p>
  );
}

function Result({ result }: { result: ProofVerifyResponse }) {
  if (result.state === "gate-failed") {
    return (
      <div
        role="status"
        className="flex flex-col gap-1 text-sm text-amber-700 dark:text-amber-300"
      >
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
      <p role="status" className="text-sm text-slate-500">
        {NOT_VERIFIABLE[result.state]}
      </p>
    );
  }
  return <VerdictResult verdict={result.verdict} stale={result.stale} />;
}

function VerdictResult({
  verdict,
  stale,
}: {
  verdict: ProofVerdictReport;
  stale: boolean;
}) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Badge label={verdict.status} tone={statusTone(verdict.status)} />
        {verdict.basis && <BasisLabel basis={verdict.basis} />}
        {verdict.reason && (
          <span className="text-xs text-slate-500">({verdict.reason})</span>
        )}
        {stale && <Badge label="prose moved" tone="warn" />}
      </div>

      <Grounds verdict={verdict} />

      <p className="text-[0.7rem] leading-snug text-slate-500">
        requirement@{verdict.provenance.requirement_revision} · subject@
        {verdict.provenance.subject_commit ?? "(not a git subject)"} · provreq@
        {verdict.provenance.tool_version}
      </p>
    </div>
  );
}

function EvidenceRow({ evidence }: { evidence: ProofEvidenceReport }) {
  return (
    <li className="flex flex-col gap-0.5">
      <div className="flex items-center gap-2">
        <Badge label={evidence.status} tone={statusTone(evidence.status)} />
        <code className="rounded bg-slate-100 px-1.5 py-0.5 text-xs dark:bg-slate-800">
          {evidence.engine}
        </code>
        {evidence.basis && <BasisLabel basis={evidence.basis} />}
      </div>
      {evidence.detail.length > 0 && (
        <ul className="ml-2 list-inside list-disc text-xs text-slate-500">
          {evidence.detail.map((d, i) => (
            <li key={i}>{d}</li>
          ))}
        </ul>
      )}
    </li>
  );
}
