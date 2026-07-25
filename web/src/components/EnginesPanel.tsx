import { useEffect, useState } from "react";
import { fetchEngines } from "../api";
import type { EngineReport, EngineState } from "../types";
import type { Tone } from "../labels";
import { Badge } from "./Badge";

// An engine that is installed but cannot start reads as a fault (warn), never as the ordinary
// "not installed here" state (muted) — the two ask the operator for different work, and toning
// them alike is how the record's distinction would get lost on the way to the screen (REQ051).
const STATE: Record<EngineState, { label: string; tone: Tone }> = {
  available: { label: "available", tone: "ok" },
  unusable: { label: "cannot start", tone: "warn" },
  incompatible: { label: "too old", tone: "warn" },
  missing: { label: "not installed", tone: "muted" },
  "not-wired": { label: "not wired", tone: "muted" },
};

export function EnginesPanel() {
  const [engines, setEngines] = useState<EngineReport[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    fetchEngines(controller.signal)
      .then(setEngines)
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        setError(err instanceof Error ? err.message : String(err));
      });
    return () => controller.abort();
  }, []);

  if (error !== null) {
    return (
      <p role="alert" className="text-xs text-warn">
        Could not probe the verification engines: {error}
      </p>
    );
  }
  if (engines === null) return null;

  const broken = engines.filter((e) => e.state === "unusable");

  return (
    <section aria-label="Verification engines" className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-muted">Engines</h2>
        {engines.map((e) => (
          <span key={e.name} className="flex items-center gap-1.5" title={e.detail}>
            <span className="text-sm">{e.name}</span>
            <Badge label={STATE[e.state].label} tone={STATE[e.state].tone} />
          </span>
        ))}
      </div>

      {broken.length > 0 && (
        <div
          role="status"
          className="rounded-lg border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn"
        >
          <ul className="flex flex-col gap-0.5">
            {broken.map((e) => (
              <li key={e.name}>
                <span className="font-medium">{e.name}</span> is installed but cannot start
                {e.reason && <> — {e.reason}</>}
              </li>
            ))}
          </ul>
          <p className="mt-1.5">
            Installing these again will not help: they are already present. Repair the environment
            they run in — a stale dev-container is the usual cause, so re-pull it.
          </p>
        </div>
      )}
    </section>
  );
}
