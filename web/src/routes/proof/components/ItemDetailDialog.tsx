import { useEffect, useId, useState } from "react";

import {
  useAdmitDraft,
  useCheckDraft,
  useDiscardDraft,
  useGroundDraft,
  useRequirement,
  useSetDraftCandidate,
  useTranslateDraft,
  useWritebackDraft,
} from "../../../api/queries";
import type { ProofDetail, ProofGateStatus } from "../../../api/types";
import { formalizationLabel, originNote, triageLabel } from "../labels";
import { Badge } from "./Badge";
import { PrlSyntaxReference } from "./PrlSyntaxReference";
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

      <CandidateEditor id={d.id} candidate={d.candidate} />

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

      {d.candidate && <GroundingForm id={d.id} />}

      {d.readback && !d.admission && (
        <AdmitControl
          id={d.id}
          mandatory={
            d.gate?.status === "passed" && d.gate.warnings.length > 0
          }
        />
      )}

      {d.admission && <WritebackControl id={d.id} stale={d.stale} />}

      <DiscardDraft id={d.id} hasCandidate={Boolean(d.candidate)} />

      <VerifyPanel id={d.id} stored={d.verdict} />
    </div>
  );
}

const BTN_PRIMARY =
  "rounded-md border border-sky-300 bg-sky-50 px-3 py-1 text-xs font-medium text-sky-800 hover:bg-sky-100 disabled:cursor-not-allowed disabled:opacity-60 dark:border-sky-800 dark:bg-sky-950/40 dark:text-sky-200 dark:hover:bg-sky-900/40";
const BTN_SECONDARY =
  "rounded-md border border-slate-300 bg-white px-3 py-1 text-xs font-medium text-slate-700 hover:border-sky-500 disabled:cursor-not-allowed disabled:opacity-60 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-200";
const FIELD_INPUT =
  "rounded-md border border-slate-300 bg-white px-2 py-1 text-xs text-slate-700 focus:border-sky-500 focus:outline-none dark:border-slate-600 dark:bg-slate-800 dark:text-slate-200";

/// Author or replace the candidate PRL and re-run the mechanical gate (REQ086), or ask the
/// configured model to forward-translate the prose into a candidate (REQ087) — the write side of
/// the read-only Candidate/Gate fields above. Saving stores the candidate (recording its gate
/// outcome and clearing any prior admission and grounding); Re-check re-gates the saved candidate;
/// Draft with a model runs the translate-then-repair loop and stores the result.
function CandidateEditor({
  id,
  candidate,
}: {
  id: string;
  candidate: string | null;
}) {
  const [prl, setPrl] = useState(candidate ?? "");
  const save = useSetDraftCandidate();
  const check = useCheckDraft();
  const translate = useTranslateDraft();
  // Re-seed when the selected item (or its stored candidate) changes underneath us — this also
  // picks up the candidate a successful translate writes into the cache.
  useEffect(() => {
    setPrl(candidate ?? "");
  }, [candidate]);
  const dirty = prl !== (candidate ?? "");
  return (
    <Field label="Candidate PRL">
      <textarea
        value={prl}
        onChange={(e) => setPrl(e.target.value)}
        rows={4}
        spellCheck={false}
        aria-label={`Candidate PRL for ${id}`}
        placeholder="Author the PRL by hand, or use “Draft with a model” to translate the prose."
        className="w-full resize-y rounded-lg border border-slate-200 bg-slate-50 p-3 font-mono text-xs dark:border-slate-800 dark:bg-slate-900"
      />
      <div className="mt-2 flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={() => save.mutate({ id, prl })}
          disabled={!prl.trim() || !dirty || save.isPending}
          className={BTN_PRIMARY}
        >
          {save.isPending ? "Saving…" : dirty ? "Save candidate" : "Saved"}
        </button>
        <button
          type="button"
          onClick={() => check.mutate({ id })}
          disabled={!candidate || dirty || check.isPending}
          className={BTN_SECONDARY}
        >
          {check.isPending ? "Checking…" : "Re-check gate"}
        </button>
        {/* Translating replaces the stored candidate, so guard unsaved hand edits like Re-check. */}
        <button
          type="button"
          onClick={() => translate.mutate({ id })}
          disabled={dirty || translate.isPending}
          className={BTN_SECONDARY}
        >
          {translate.isPending ? "Translating…" : "Draft with a model"}
        </button>
        {dirty && (
          <span className="text-xs text-amber-600 dark:text-amber-400">
            unsaved edits
          </span>
        )}
      </div>
      {(save.isError || check.isError || translate.isError) && (
        <p role="alert" className="mt-1 text-xs text-amber-700 dark:text-amber-300">
          {String(save.error ?? check.error ?? translate.error)}
        </p>
      )}
      <div className="mt-2">
        <PrlSyntaxReference />
      </div>
    </Field>
  );
}

/// Attach a grounding binding (REQ086). Shown only once a candidate exists, since the symbol is
/// validated against the candidate's declared vocabulary — a symbol it does not speak of is rejected.
function GroundingForm({ id }: { id: string }) {
  const [symbol, setSymbol] = useState("");
  const [observable, setObservable] = useState("");
  const [fidelity, setFidelity] = useState("");
  const ground = useGroundDraft();
  const submit = () => {
    ground.mutate(
      {
        id,
        symbol: symbol.trim(),
        observable: observable.trim(),
        fidelity: fidelity || undefined,
      },
      {
        onSuccess: () => {
          setSymbol("");
          setObservable("");
          setFidelity("");
        },
      },
    );
  };
  return (
    <Field label="Add grounding">
      <div className="flex flex-wrap items-center gap-2">
        <input
          value={symbol}
          onChange={(e) => setSymbol(e.target.value)}
          placeholder="symbol"
          aria-label="symbol"
          className={FIELD_INPUT}
        />
        <span aria-hidden className="text-slate-500">
          →
        </span>
        <input
          value={observable}
          onChange={(e) => setObservable(e.target.value)}
          placeholder="observable"
          aria-label="observable"
          className={FIELD_INPUT}
        />
        <select
          value={fidelity}
          onChange={(e) => setFidelity(e.target.value)}
          aria-label="fidelity"
          className={FIELD_INPUT}
        >
          <option value="">default fidelity</option>
          <option value="definitional">definitional</option>
          <option value="observed">observed</option>
          <option value="probed">probed</option>
        </select>
        <button
          type="button"
          onClick={submit}
          disabled={!symbol.trim() || !observable.trim() || ground.isPending}
          className={BTN_PRIMARY}
        >
          {ground.isPending ? "Binding…" : "Bind"}
        </button>
      </div>
      {ground.isError && (
        <p role="alert" className="mt-1 text-xs text-amber-700 dark:text-amber-300">
          {String(ground.error)}
        </p>
      )}
    </Field>
  );
}

/// Discard the whole draft (REQ086). Only offered when there is a draft to discard; confirmed first
/// because it drops the candidate, its gate, and every grounding binding.
function DiscardDraft({
  id,
  hasCandidate,
}: {
  id: string;
  hasCandidate: boolean;
}) {
  const discard = useDiscardDraft();
  if (!hasCandidate) return null;
  return (
    <div>
      <button
        type="button"
        onClick={() => {
          if (
            window.confirm(
              "Discard this draft? Its candidate PRL, gate, and grounding are removed.",
            )
          ) {
            discard.mutate({ id });
          }
        }}
        disabled={discard.isPending}
        className="text-xs font-medium text-rose-600 hover:underline disabled:opacity-60 dark:text-rose-400"
      >
        {discard.isPending ? "Discarding…" : "Discard draft"}
      </button>
      {discard.isError && (
        <p role="alert" className="mt-1 text-xs text-amber-700 dark:text-amber-300">
          {String(discard.error)}
        </p>
      )}
    </div>
  );
}

/// Admit the draft's formalization (REQ088). The Read-back field above is what the operator is
/// confirming; a mandatory-review (vacuity-flagged) candidate requires ticking the confirmation
/// before Admit enables — the UI's equivalent of the command line's prompt. The reviewer name is
/// recorded as provenance, so it is required.
function AdmitControl({ id, mandatory }: { id: string; mandatory: boolean }) {
  const [reviewer, setReviewer] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const admit = useAdmitDraft();
  const canAdmit = reviewer.trim().length > 0 && (!mandatory || confirmed);
  return (
    <Field label="Admit">
      <div className="flex flex-wrap items-center gap-2">
        <input
          type="text"
          value={reviewer}
          onChange={(e) => setReviewer(e.target.value)}
          placeholder="reviewer"
          aria-label={`Reviewer admitting ${id}`}
          className={FIELD_INPUT}
        />
        <button
          type="button"
          onClick={() =>
            admit.mutate({ id, reviewer: reviewer.trim(), confirmed })
          }
          disabled={!canAdmit || admit.isPending}
          className={BTN_PRIMARY}
        >
          {admit.isPending ? "Admitting…" : "Admit"}
        </button>
      </div>
      {mandatory && (
        <label className="mt-2 flex items-center gap-2 text-xs text-amber-700 dark:text-amber-300">
          <input
            type="checkbox"
            checked={confirmed}
            onChange={(e) => setConfirmed(e.target.checked)}
          />
          Vacuity-flagged — I confirm the read-back above matches intent.
        </label>
      )}
      {admit.isError && (
        <p role="alert" className="mt-1 text-xs text-amber-700 dark:text-amber-300">
          {String(admit.error)}
        </p>
      )}
    </Field>
  );
}

/// Write the admitted provenance onto the subject's source file (REQ088) — the only draft action
/// that mutates the subject. Disabled when the prose has moved since admission (the backend refuses
/// a stale write-back); the operator re-admits first. Confirmed because it changes tracked files.
function WritebackControl({ id, stale }: { id: string; stale: boolean }) {
  const writeback = useWritebackDraft();
  return (
    <Field label="Write back to source">
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={() => {
            if (
              window.confirm(
                "Write this formalization's provenance onto the requirement's source file? Review and commit the working-tree change yourself.",
              )
            ) {
              writeback.mutate({ id });
            }
          }}
          disabled={stale || writeback.isPending}
          className={BTN_PRIMARY}
        >
          {writeback.isPending ? "Writing…" : "Write back to source…"}
        </button>
        {stale && (
          <span className="text-xs text-amber-600 dark:text-amber-400">
            prose moved — re-admit before writing back
          </span>
        )}
      </div>
      {writeback.isError && (
        <p role="alert" className="mt-1 text-xs text-amber-700 dark:text-amber-300">
          {String(writeback.error)}
        </p>
      )}
    </Field>
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
