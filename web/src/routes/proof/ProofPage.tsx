import { useState } from "react";
import clsx from "clsx";

import { api } from "../../api/client";
import {
  queryKeys,
  useRequirements,
  useTriageRequirement,
} from "../../api/queries";
import { useQueryClient } from "@tanstack/react-query";
import type {
  ProofBacklog,
  ProofClassification,
  ProofItemState,
} from "../../api/types";
import { CoverageBar } from "./components/CoverageBar";
import { EnginesPanel } from "./components/EnginesPanel";
import { ItemDetailDialog } from "./components/ItemDetailDialog";
import { RequirementsTable } from "./components/RequirementsTable";

type Filter = {
  key: string;
  label: string;
  match: (item: ProofItemState) => boolean;
};

const FILTERS: Filter[] = [
  { key: "all", label: "All", match: () => true },
  {
    key: "formalizable",
    label: "Formalizable",
    match: (i) => i.classification === "formalizable-now",
  },
  {
    key: "drafting",
    label: "In progress",
    match: (i) => i.formalization === "drafting",
  },
  {
    key: "admitted",
    label: "Formalized",
    match: (i) => i.formalization === "admitted",
  },
  { key: "untriaged", label: "Untriaged", match: (i) => i.classification === null },
];

/// provreq's distinguishing surface: the requirement backlog, its coverage
/// funnel, the live engine health, per-item triage, and on-demand verification.
/// Grafted into the ReqForge management frontend as `/proof` and restyled to the
/// slate palette; the data flows through react-query like every other page.
export function ProofPage() {
  const backlog = useRequirements();

  return (
    <section className="space-y-6">
      <header className="space-y-1 border-b border-slate-200 pb-3 dark:border-slate-800">
        <h1 className="text-2xl font-semibold tracking-tight">Proof</h1>
        <p className="text-sm text-slate-600 dark:text-slate-400">
          The requirement backlog and its formalization coverage. Triage each
          item, then verify the formalized ones against the engine ensemble.
        </p>
      </header>

      {/* Above the backlog on purpose (REQ051): what an engine can prove right
          now conditions every verdict below it. Independent of the backlog
          fetch, so it renders even when the subject is unadopted (409). */}
      <EnginesPanel />

      {backlog.isLoading ? (
        <p role="status" className="text-sm text-slate-500">
          Loading backlog…
        </p>
      ) : backlog.isError || !backlog.data ? (
        <p
          role="alert"
          className="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800 dark:border-amber-800 dark:bg-amber-950/30 dark:text-amber-200"
        >
          {String(backlog.error ?? "Failed to load backlog")}
        </p>
      ) : (
        <BacklogView backlog={backlog.data} />
      )}
    </section>
  );
}

type ReverifyProgress = { done: number; total: number };

function BacklogView({ backlog }: { backlog: ProofBacklog }) {
  const qc = useQueryClient();
  const triage = useTriageRequirement();
  const [filter, setFilter] = useState("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [reverifying, setReverifying] = useState<ReverifyProgress | null>(null);
  const [reverifyError, setReverifyError] = useState<string | null>(null);

  const active = FILTERS.find((f) => f.key === filter) ?? FILTERS[0];
  const shown = backlog.items.filter(active.match);

  const handleTriage = (id: string, classification: ProofClassification) => {
    triage.mutate({ id, classification });
  };

  // Re-verify every drifted verdict, then refresh the funnel so the counts
  // settle (REQ044). The stale set is read straight off the backlog the server
  // already computed — a fresh verdict is never re-run. One item's failure never
  // aborts the rest; all are surfaced together at the end. Sequential on
  // purpose: each verify runs the heavy prover ensemble server-side, and firing
  // them in parallel would thrash CPU on a single-operator loopback tool.
  const handleReverifyStale = async () => {
    const staleIds = backlog.items
      .filter((i) => i.verdict && !i.verdict.fresh)
      .map((i) => i.id);
    if (staleIds.length === 0) return;
    setReverifyError(null);
    setReverifying({ done: 0, total: staleIds.length });
    const failures: string[] = [];
    for (const id of staleIds) {
      try {
        await api.verifyRequirement(id);
      } catch (err: unknown) {
        failures.push(`${id}: ${err instanceof Error ? err.message : String(err)}`);
      }
      setReverifying((r) => (r ? { ...r, done: r.done + 1 } : r));
    }
    await qc.invalidateQueries({ queryKey: queryKeys.requirements });
    setReverifying(null);
    if (failures.length > 0) {
      setReverifyError(`Some re-verifications failed — ${failures.join("; ")}`);
    }
  };

  return (
    <div className="flex flex-col gap-8">
      {triage.isError && (
        <p
          role="alert"
          className="rounded-lg border border-amber-300 bg-amber-50 px-4 py-2 text-sm text-amber-800 dark:border-amber-800 dark:bg-amber-950/30 dark:text-amber-200"
        >
          Could not save triage: {String(triage.error)}
        </p>
      )}
      {reverifyError && (
        <p
          role="alert"
          className="rounded-lg border border-amber-300 bg-amber-50 px-4 py-2 text-sm text-amber-800 dark:border-amber-800 dark:bg-amber-950/30 dark:text-amber-200"
        >
          {reverifyError}
        </p>
      )}

      <div className="rounded-xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-800 dark:bg-slate-900">
        <CoverageBar coverage={backlog.coverage} />
        {backlog.coverage.stale > 0 && (
          <div className="mt-4 flex flex-wrap items-center gap-3 border-t border-slate-200 pt-4 dark:border-slate-800">
            <button
              type="button"
              onClick={handleReverifyStale}
              disabled={reverifying !== null}
              className="rounded-md border border-amber-300 bg-amber-50 px-3 py-1 text-sm font-medium text-amber-800 hover:bg-amber-100 disabled:cursor-not-allowed disabled:opacity-60 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-200 dark:hover:bg-amber-900/40"
            >
              {reverifying
                ? `Re-verifying ${reverifying.done}/${reverifying.total}…`
                : `Re-verify all stale (${backlog.coverage.stale})`}
            </button>
            <span className="text-xs text-slate-500">
              Re-runs the engines on every drifted verdict.
            </span>
          </div>
        )}
      </div>

      <div className="flex flex-col gap-4">
        <div
          role="tablist"
          aria-label="Filter requirements"
          className="flex flex-wrap gap-1 border-b border-slate-200 dark:border-slate-800"
        >
          {FILTERS.map((f) => {
            const isActive = f.key === filter;
            return (
              <button
                key={f.key}
                type="button"
                role="tab"
                aria-selected={isActive}
                onClick={() => setFilter(f.key)}
                className={clsx(
                  "-mb-px rounded-t-md border-b-2 px-3 py-1.5 text-sm transition-colors",
                  isActive
                    ? "border-sky-500 font-medium text-slate-900 dark:text-slate-100"
                    : "border-transparent text-slate-500 hover:text-slate-900 dark:hover:text-slate-200",
                )}
              >
                {f.label}
              </button>
            );
          })}
        </div>
        <div className="rounded-xl border border-slate-200 bg-white p-2 dark:border-slate-800 dark:bg-slate-900">
          <RequirementsTable
            items={shown}
            onSelect={setSelectedId}
            onTriage={handleTriage}
          />
        </div>
      </div>

      <ItemDetailDialog id={selectedId} onClose={() => setSelectedId(null)} />
    </div>
  );
}
