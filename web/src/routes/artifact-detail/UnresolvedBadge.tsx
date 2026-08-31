import type { LinkHint } from "../../api/types";

/// The "unresolved — <slug>" affordance per TRACE-unresolvedLinks.
/// In the single-subject world there are no other mounts to
/// initialise, so the copy is purely informational: it names the
/// project slug the link points at that the backend could not
/// resolve.
export function UnresolvedBadge({ hint }: { hint: LinkHint }) {
  return (
    <span className="ml-2 text-xs text-slate-500">
      unresolved — mount{" "}
      <code className="font-mono text-slate-700 dark:text-slate-300">
        {hint.projectSlug}
      </code>
    </span>
  );
}
