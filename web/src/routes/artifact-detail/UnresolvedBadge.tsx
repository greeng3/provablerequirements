import { Link as RouterLink } from "react-router-dom";

import { useMounts } from "../../api/queries";
import type { LinkHint } from "../../api/types";

/// The "unresolved — mount <slug>" affordance per
/// TRACE-unresolvedLinks. When the hinted project slug happens to
/// match a currently-mounted-but-NeedsInit directory, we offer a
/// shortcut to the System Home so the operator can initialise the
/// mount in one click instead of hunting for it. If nothing
/// matches, the copy is informational only.
export function UnresolvedBadge({ hint }: { hint: LinkHint }) {
  const mountsQuery = useMounts();
  const needsInit =
    mountsQuery.data?.find(
      (m) => m.state === "needsInit" && m.dirName === hint.projectSlug,
    ) ?? null;

  return (
    <span className="ml-2 text-xs text-slate-500">
      unresolved — mount{" "}
      <code className="font-mono text-slate-700 dark:text-slate-300">
        {hint.projectSlug}
      </code>
      {needsInit ? (
        <>
          {" · "}
          <RouterLink
            to="/"
            state={{ highlightMount: needsInit.dirName }}
            className="underline hover:text-slate-700 dark:hover:text-slate-200"
          >
            init this mount
          </RouterLink>
        </>
      ) : null}
    </span>
  );
}
