import { useEffect, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { fetchDetail } from "../api";
import type { Detail, GateStatus } from "../types";
import * as labels from "../labels";
import { Badge } from "./Badge";

type Props = {
  id: string | null;
  onClose: () => void;
};

type State =
  | { kind: "loading" }
  | { kind: "ready"; detail: Detail }
  | { kind: "error"; message: string };

export function ItemDetailDialog({ id, onClose }: Props) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    if (id === null) return;
    setState({ kind: "loading" });
    const controller = new AbortController();
    fetchDetail(id, controller.signal)
      .then((detail) => setState({ kind: "ready", detail }))
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        setState({ kind: "error", message: err instanceof Error ? err.message : String(err) });
      });
    return () => controller.abort();
  }, [id]);

  return (
    <Dialog.Root open={id !== null} onOpenChange={(open) => !open && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/40 backdrop-blur-sm" />
        <Dialog.Content
          aria-describedby={undefined}
          className="fixed left-1/2 top-1/2 max-h-[85vh] w-[min(92vw,42rem)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-xl border border-border bg-surface p-6 shadow-xl focus:outline-none"
        >
          <Body state={state} />
          <Dialog.Close
            aria-label="Close"
            className="absolute right-4 top-4 rounded-md px-2 py-1 text-muted hover:bg-surface-2 hover:text-text"
          >
            ✕
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function Body({ state }: { state: State }) {
  if (state.kind === "loading") {
    return (
      <>
        <Dialog.Title className="text-lg font-semibold">Loading…</Dialog.Title>
        <p role="status" className="mt-2 text-muted">
          Loading requirement detail…
        </p>
      </>
    );
  }
  if (state.kind === "error") {
    return (
      <>
        <Dialog.Title className="text-lg font-semibold">Unavailable</Dialog.Title>
        <p role="alert" className="mt-2 text-warn">
          {state.message}
        </p>
      </>
    );
  }

  const d = state.detail;
  const triage = labels.triage(d.classification);
  const formal = labels.formalization(d.formalization);
  return (
    <div className="flex flex-col gap-5">
      <header className="flex flex-col gap-2 pr-8">
        <Dialog.Title className="text-xl font-bold tabular-nums">{d.id}</Dialog.Title>
        {d.title && <p className="text-muted">{d.title}</p>}
        <div className="flex flex-wrap items-center gap-2">
          <Badge label={triage.label} tone={triage.tone} />
          <Badge label={formal.label} tone={formal.tone} />
          {d.stale && <Badge label="prose moved" tone="warn" />}
          {d.admission && (
            <span className="text-xs text-muted">
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
          <pre className="overflow-x-auto rounded-lg border border-border bg-surface-2 p-3 text-xs">
            {d.candidate}
          </pre>
        </Field>
      ) : (
        <p className="text-sm text-muted">Not formalized yet — no candidate PRL.</p>
      )}

      {d.gate && <GateView gate={d.gate} />}

      {d.readback && (
        <Field label="Read-back">
          <p className="max-w-prose text-sm italic leading-relaxed text-muted">{d.readback}</p>
        </Field>
      )}

      {d.bindings.length > 0 && (
        <Field label="Grounding">
          <ul className="flex flex-col gap-1 text-sm">
            {d.bindings.map((b) => (
              <li key={b.symbol} className="flex items-center gap-2">
                <code className="rounded bg-surface-2 px-1.5 py-0.5 text-xs">{b.symbol}</code>
                <span className="text-muted">→</span>
                <code className="rounded bg-surface-2 px-1.5 py-0.5 text-xs">{b.observable}</code>
                <span className="text-xs text-muted">({b.fidelity})</span>
              </li>
            ))}
          </ul>
        </Field>
      )}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="text-xs font-semibold uppercase tracking-wide text-muted">{label}</h3>
      {children}
    </section>
  );
}

function GateView({ gate }: { gate: GateStatus }) {
  const messages = gate.status === "passed" ? gate.warnings : gate.status === "failed" ? gate.errors : [];
  const tone = gate.status === "passed" ? "ok" : gate.status === "failed" ? "warn" : "muted";
  return (
    <Field label="Gate">
      <div className="flex flex-col gap-1.5">
        <Badge label={gate.status} tone={tone} />
        {messages.length > 0 && (
          <ul className="ml-1 list-inside list-disc text-xs text-muted">
            {messages.map((m, i) => (
              <li key={i}>{m}</li>
            ))}
          </ul>
        )}
      </div>
    </Field>
  );
}
