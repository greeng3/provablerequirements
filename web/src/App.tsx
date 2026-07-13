import { useEffect, useState } from "react";

interface Health {
  status: string;
  version: string;
}

type State =
  | { kind: "loading" }
  | { kind: "ready"; health: Health }
  | { kind: "error"; message: string };

export function App() {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    fetch("/health")
      .then((res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json() as Promise<Health>;
      })
      .then((health) => {
        if (!cancelled) setState({ kind: "ready", health });
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          const message = err instanceof Error ? err.message : String(err);
          setState({ kind: "error", message });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="shell">
      <section className="card" aria-labelledby="title">
        <h1 id="title">provreq</h1>
        <p className="tagline">PRL native provisioner &amp; backend</p>
        <Status state={state} />
      </section>
    </main>
  );
}

function Status({ state }: { state: State }) {
  switch (state.kind) {
    case "loading":
      return <p role="status">Checking backend…</p>;
    case "error":
      return (
        <p role="alert" className="error">
          Backend unreachable: {state.message}
        </p>
      );
    case "ready":
      return (
        <dl className="health">
          <dt>Status</dt>
          <dd className="ok">{state.health.status}</dd>
          <dt>Version</dt>
          <dd>{state.health.version}</dd>
        </dl>
      );
  }
}
