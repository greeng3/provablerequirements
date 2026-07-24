import { useEffect, useState } from "react";
import * as Tabs from "@radix-ui/react-tabs";
import { fetchBacklog, setTriage, verifyRequirement } from "./api";
import type { Backlog, Classification, ItemState } from "./types";
import { CoverageBar } from "./components/CoverageBar";
import { RequirementsTable } from "./components/RequirementsTable";
import { ItemDetailDialog } from "./components/ItemDetailDialog";

type State =
  | { kind: "loading" }
  | { kind: "ready"; backlog: Backlog }
  | { kind: "error"; message: string };

type Filter = { key: string; label: string; match: (item: ItemState) => boolean };

const FILTERS: Filter[] = [
  { key: "all", label: "All", match: () => true },
  { key: "formalizable", label: "Formalizable", match: (i) => i.classification === "formalizable-now" },
  { key: "drafting", label: "In progress", match: (i) => i.formalization === "drafting" },
  { key: "admitted", label: "Formalized", match: (i) => i.formalization === "admitted" },
  { key: "untriaged", label: "Untriaged", match: (i) => i.classification === null },
];

export function App() {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    const controller = new AbortController();
    fetchBacklog(controller.signal)
      .then((backlog) => setState({ kind: "ready", backlog }))
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        const message = err instanceof Error ? err.message : String(err);
        setState({ kind: "error", message });
      });
    return () => controller.abort();
  }, []);

  return (
    <div className="min-h-screen">
      <div className="mx-auto max-w-4xl px-6 py-10">
        <header className="mb-8">
          <h1 className="text-2xl font-bold tracking-tight">provreq</h1>
          <p className="text-muted">Requirement backlog &amp; coverage</p>
        </header>
        <Body state={state} />
      </div>
    </div>
  );
}

function Body({ state }: { state: State }) {
  switch (state.kind) {
    case "loading":
      return (
        <p role="status" className="text-muted">
          Loading backlog…
        </p>
      );
    case "error":
      return (
        <p
          role="alert"
          className="rounded-lg border border-warn/40 bg-warn/10 px-4 py-3 text-warn"
        >
          {state.message}
        </p>
      );
    case "ready":
      return <Backlog backlog={state.backlog} />;
  }
}

function Backlog({ backlog }: { backlog: Backlog }) {
  const [data, setData] = useState(backlog);
  const [filter, setFilter] = useState("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reverifying, setReverifying] = useState<{ done: number; total: number } | null>(null);
  const active = FILTERS.find((f) => f.key === filter) ?? FILTERS[0];
  const shown = data.items.filter(active.match);

  async function handleTriage(id: string, classification: Classification) {
    const previous = data;
    setError(null);
    // Optimistic: reflect the new bucket immediately, then reconcile with the authoritative
    // backlog the server returns (correct coverage); roll back and surface the error on failure.
    setData((d) => ({
      ...d,
      items: d.items.map((it) => (it.id === id ? { ...it, classification } : it)),
    }));
    try {
      setData(await setTriage(id, classification));
    } catch (err: unknown) {
      setData(previous);
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  // Re-verify every drifted verdict, then refresh the funnel so the counts settle (REQ044). The
  // stale set is read straight off the backlog the server already computed — a fresh verdict is
  // never re-run. One item's failure never aborts the rest; all are surfaced together at the end.
  async function handleReverifyStale() {
    const staleIds = data.items.filter((i) => i.verdict && !i.verdict.fresh).map((i) => i.id);
    if (staleIds.length === 0) return;
    setError(null);
    setReverifying({ done: 0, total: staleIds.length });
    const failures: string[] = [];
    // ponytail: sequential — each verify runs the heavy prover ensemble server-side; firing them
    // in parallel would thrash CPU on a single-operator loopback tool. Parallelize only if a real
    // backlog makes this slow.
    for (const id of staleIds) {
      try {
        await verifyRequirement(id);
      } catch (err: unknown) {
        failures.push(`${id}: ${err instanceof Error ? err.message : String(err)}`);
      }
      setReverifying((r) => (r ? { ...r, done: r.done + 1 } : r));
    }
    try {
      setData(await fetchBacklog());
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
    setReverifying(null);
    if (failures.length > 0) {
      setError(`Some re-verifications failed — ${failures.join("; ")}`);
    }
  }

  return (
    <div className="flex flex-col gap-8">
      {error && (
        <p role="alert" className="rounded-lg border border-warn/40 bg-warn/10 px-4 py-2 text-sm text-warn">
          Could not save triage: {error}
        </p>
      )}
      <div className="rounded-xl border border-border bg-surface p-5 shadow-sm">
        <CoverageBar coverage={data.coverage} />
        {data.coverage.stale > 0 && (
          <div className="mt-4 flex flex-wrap items-center gap-3 border-t border-border pt-4">
            <button
              type="button"
              onClick={handleReverifyStale}
              disabled={reverifying !== null}
              className="rounded-md border border-warn/40 bg-warn/10 px-3 py-1 text-sm font-medium text-warn hover:bg-warn/20 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {reverifying
                ? `Re-verifying ${reverifying.done}/${reverifying.total}…`
                : `Re-verify all stale (${data.coverage.stale})`}
            </button>
            <span className="text-xs text-muted">
              Re-runs the engines on every drifted verdict.
            </span>
          </div>
        )}
      </div>

      <Tabs.Root value={filter} onValueChange={setFilter}>
        <Tabs.List
          aria-label="Filter requirements"
          className="mb-4 flex flex-wrap gap-1 border-b border-border"
        >
          {FILTERS.map((f) => (
            <Tabs.Trigger
              key={f.key}
              value={f.key}
              className="rounded-t-md px-3 py-1.5 text-sm text-muted transition-colors hover:text-text data-[state=active]:border-b-2 data-[state=active]:border-accent data-[state=active]:text-text"
            >
              {f.label}
            </Tabs.Trigger>
          ))}
        </Tabs.List>
        <Tabs.Content value={filter} className="rounded-xl border border-border bg-surface p-2">
          <RequirementsTable items={shown} onSelect={setSelectedId} onTriage={handleTriage} />
        </Tabs.Content>
      </Tabs.Root>

      <ItemDetailDialog id={selectedId} onClose={() => setSelectedId(null)} />
    </div>
  );
}
