import type { ProofVerdictView } from "../../../api/types";
import type { Tone } from "../labels";
import { Badge } from "./Badge";

// The stored-verdict polarity tone, matching the freshly-run VerifyPanel: holds
// calm, fails warns, unknown muted (no answer, never dressed up as either).
const STATUS_TONE: Record<string, Tone> = {
  holds: "ok",
  fails: "warn",
  unknown: "muted",
};

type Props = {
  verdict: ProofVerdictView;
};

/// A compact stored-verdict pill: the polarity plus, when the verdict has
/// drifted, a "stale" marker whose tooltip names why (REQ039). A stale verdict
/// is never hidden — the operator sees it and its re-verify prompt.
export function VerdictBadge({ verdict }: Props) {
  const tone = STATUS_TONE[verdict.status] ?? "muted";
  return (
    <span className="inline-flex items-center gap-1.5">
      <Badge label={verdict.status} tone={tone} />
      {!verdict.fresh && (
        <span
          className="cursor-help text-xs font-medium text-amber-700 dark:text-amber-300"
          title={verdict.stale_reasons.join("\n")}
        >
          ⟳ stale
        </span>
      )}
    </span>
  );
}
