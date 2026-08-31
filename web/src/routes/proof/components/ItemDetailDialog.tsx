import { useEffect, useId } from "react";

import { useRequirement } from "../../../api/queries";
import type { ProofDetail, ProofGateStatus } from "../../../api/types";
import { formalizationLabel, originNote, triageLabel } from "../labels";
import { Badge } from "./Badge";
import { VerifyPanel } from "./VerifyPanel";

type Props = {
  id: string | null;
  onClose: () => void;
};

/// The read-only formalization detail for one requirement (REQ035), shown in a
/// hand-rolled modal matching the management frontend's dialog shell
/// (LinkCreateDialog): overlay click and Escape both close, no Radix.
export function ItemDetailDialog({ id, onClose }: Props) {
  const query = useRequirement(id);
  const headingId = useId();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  if (id === null) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={headingId}
      className="fixed inset-0 z-10 flex items-start justify-center overflow-y-auto bg-black/40 p-4 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="my-8 w-full max-w-2xl rounded-xl border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="relative">
          <button
            type="button"
            aria-label="Close"
            onClick={onClose}
            className="absolute right-0 top-0 rounded-md px-2 py-1 text-slate-500 hover:bg-slate-100 hover:text-slate-900 dark:hover:bg-slate-800 dark:hover:text-slate-100"
          >
            ✕
          </button>
          <Body query={query} headingId={headingId} />
        </div>
      </div>
    </div>
  );
}

type BodyProps = {
  query: ReturnType<typeof useRequirement>;
  headingId: string;
};

function Body({ query, headingId }: BodyProps) {
  if (query.isLoading) {
    return (
      <>
        <h2 id={headingId} className="text-lg font-semibold">
          Loading…
        </h2>
        <p role="status" className="mt-2 text-slate-500">
          Loading requirement detail…
        </p>
      </>
    );
  }
  if (query.isError || !query.data) {
    return (
      <>
        <h2 id={headingId} className="text-lg font-semibold">
          Unavailable
        </h2>
        <p role="alert" className="mt-2 text-amber-700 dark:text-amber-300">
          {String(query.error ?? "unknown error")}
        </p>
      </>
    );
  }
  return <DetailView detail={query.data} headingId={headingId} />;
}

function DetailView({
  detail: d,
  headingId,
}: {
  detail: ProofDetail;
  headingId: string;
}) {
  const triage = triageLabel(d.classification);
  const formal = formalizationLabel(d.formalization);
  const origin = originNote(d.classified_by);
  return (
    <div className="flex flex-col gap-5">
      <header className="flex flex-col gap-2 pr-8">
        <h2 id={headingId} className="text-xl font-bold tabular-nums">
          {d.id}
        </h2>
        {d.title && <p className="text-slate-500">{d.title}</p>}
        <div className="flex flex-wrap items-center gap-2">
          <Badge label={triage.label} tone={triage.tone} />
          {/* Only origins worth less than the bucket looks are annotated (#180). */}
          {origin && (
            <span className="text-xs italic text-slate-500">{origin}</span>
          )}
          <Badge label={formal.label} tone={formal.tone} />
          {d.stale && <Badge label="prose moved" tone="warn" />}
          {d.admission && (
            <span className="text-xs text-slate-500">
              admitted by {d.admission.by} · {d.admission.review} review
            </span>
          )}
        </div>
      </header>

      <Field label="Requirement">
        <p className="max-w-prose text-sm leading-relaxed">{d.text}</p>
      </Field>

      {d.candidate ? (
        <Field label="Candidate PRL">
          <pre className="overflow-x-auto rounded-lg border border-slate-200 bg-slate-50 p-3 text-xs dark:border-slate-800 dark:bg-slate-900">
            {d.candidate}
          </pre>
        </Field>
      ) : (
        <p className="text-sm text-slate-500">
          Not formalized yet — no candidate PRL.
        </p>
      )}

      {d.gate && <GateView gate={d.gate} />}

      {d.readback && (
        <Field label="Read-back">
          <p className="max-w-prose text-sm italic leading-relaxed text-slate-500">
            {d.readback}
          </p>
        </Field>
      )}

      {d.grounding ? (
        <Field label="Grounding">
          <div className="mb-2">
            <Badge
              label={d.grounding.grounded ? "grounded" : "parked"}
              tone={d.grounding.grounded ? "ok" : "warn"}
            />
          </div>
          <ul className="flex flex-col gap-2 text-sm">
            {d.grounding.bindings.map((b) => (
              <li key={b.symbol} className="flex flex-col gap-0.5">
                <div className="flex items-center gap-2">
                  <span
                    aria-hidden
                    className={
                      b.resolved
                        ? "text-emerald-600 dark:text-emerald-400"
                        : "text-amber-600 dark:text-amber-400"
                    }
                  >
                    {b.resolved ? "✓" : "✗"}
                  </span>
                  <code className="rounded bg-slate-100 px-1.5 py-0.5 text-xs dark:bg-slate-800">
                    {b.symbol}
                  </code>
                  <span className="text-slate-500">→</span>
                  <code className="rounded bg-slate-100 px-1.5 py-0.5 text-xs dark:bg-slate-800">
                    {b.observable}
                  </code>
                </div>
                <p className="ml-6 text-xs leading-snug text-slate-500">
                  {b.summary}
                </p>
              </li>
            ))}
          </ul>
        </Field>
      ) : (
        d.bindings.length > 0 && (
          <Field label="Grounding">
            <ul className="flex flex-col gap-1 text-sm">
              {d.bindings.map((b) => (
                <li key={b.symbol} className="flex items-center gap-2">
                  <code className="rounded bg-slate-100 px-1.5 py-0.5 text-xs dark:bg-slate-800">
                    {b.symbol}
                  </code>
                  <span className="text-slate-500">→</span>
                  <code className="rounded bg-slate-100 px-1.5 py-0.5 text-xs dark:bg-slate-800">
                    {b.observable}
                  </code>
                  <span className="text-xs text-slate-500">({b.fidelity})</span>
                </li>
              ))}
            </ul>
          </Field>
        )
      )}

      <VerifyPanel id={d.id} stored={d.verdict} />
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500">
        {label}
      </h3>
      {children}
    </section>
  );
}

function GateView({ gate }: { gate: ProofGateStatus }) {
  const messages =
    gate.status === "passed"
      ? gate.warnings
      : gate.status === "failed"
        ? gate.errors
        : [];
  const tone =
    gate.status === "passed" ? "ok" : gate.status === "failed" ? "warn" : "muted";
  return (
    <Field label="Gate">
      <div className="flex flex-col gap-1.5">
        <Badge label={gate.status} tone={tone} />
        {messages.length > 0 && (
          <ul className="ml-1 list-inside list-disc text-xs text-slate-500">
            {messages.map((m, i) => (
              <li key={i}>{m}</li>
            ))}
          </ul>
        )}
      </div>
    </Field>
  );
}
