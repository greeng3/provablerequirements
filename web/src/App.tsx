import { useEffect, useState } from "react";
import * as Tabs from "@radix-ui/react-tabs";
import { fetchBacklog } from "./api";
import type { Backlog, ItemState } from "./types";
import { CoverageBar } from "./components/CoverageBar";
import { RequirementsTable } from "./components/RequirementsTable";

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
  const [filter, setFilter] = useState("all");
  const active = FILTERS.find((f) => f.key === filter) ?? FILTERS[0];
  const shown = backlog.items.filter(active.match);

  return (
    <div className="flex flex-col gap-8">
      <div className="rounded-xl border border-border bg-surface p-5 shadow-sm">
        <CoverageBar coverage={backlog.coverage} />
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
          <RequirementsTable items={shown} />
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
}
