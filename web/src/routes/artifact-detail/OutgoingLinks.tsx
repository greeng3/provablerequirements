import { Link as RouterLink } from "react-router-dom";
import clsx from "clsx";

import type { LinkView } from "../../api/types";
import { LinkTypeBadge } from "./LinkTypeBadge";
import { UnresolvedBadge } from "./UnresolvedBadge";

/// Outgoing typed links, grouped by link type. Each link renders
/// its resolution state per TRACE-unresolvedLinks +
/// TRACE-linkExtensibility:
/// - resolved: click-through to the target artifact.
/// - unresolved: hint + "mount <slug>" affordance (target unknown).
/// - unknownType: link rendered but type flagged as outside the
///   effective catalog.
/// When `onRemove` is provided, each link gets a remove button
/// used by ArtifactEditor's staged edit mode.
export function OutgoingLinks({
  links,
  onRemove,
}: {
  links: LinkView[];
  onRemove?: (index: number) => void;
}) {
  if (links.length === 0) {
    return <p className="text-sm text-slate-500">No outgoing links.</p>;
  }

  const indexed = links.map((link, index) => ({ link, index }));
  const grouped = groupByType(indexed);
  const entries = Object.entries(grouped).sort(([a], [b]) =>
    a.localeCompare(b),
  );

  return (
    <ul className="space-y-3" aria-label="Outgoing links">
      {entries.map(([type, items]) => (
        <li key={type}>
          <p className="text-xs font-semibold tracking-wide text-slate-500 uppercase">
            {type}
          </p>
          <ul className="mt-1 space-y-1 text-sm">
            {items.map(({ link, index }) => (
              <li
                key={`${link.type}:${link.targetUuid}:${index}`}
                className="flex items-center gap-2"
              >
                <LinkTypeBadge
                  typeName={link.type}
                  metadata={link.typeMetadata}
                />
                <LinkBody link={link} />
                {onRemove && (
                  <button
                    type="button"
                    onClick={() => onRemove(index)}
                    aria-label={`Remove link to ${link.hint.artifactName}`}
                    className="ml-auto text-xs text-slate-500 hover:text-rose-600 dark:hover:text-rose-400"
                  >
                    Remove
                  </button>
                )}
              </li>
            ))}
          </ul>
        </li>
      ))}
    </ul>
  );
}

function LinkBody({ link }: { link: LinkView }) {
  if (link.resolution === "resolved") {
    const displayName =
      link.targetSummary?.artifactName ?? link.hint.artifactName;
    return (
      <RouterLink
        to={`/artifacts/${link.targetUuid}`}
        className="text-slate-800 hover:underline dark:text-slate-200"
      >
        <span className="font-mono text-slate-500">
          {link.hint.projectSlug}/{link.hint.collectionPrefix}/
        </span>
        {displayName}
        {link.targetSummary?.title && (
          <span className="ml-1 text-slate-500">
            — {link.targetSummary.title}
          </span>
        )}
      </RouterLink>
    );
  }

  if (link.resolution === "unknownType") {
    return (
      <span className="text-slate-700 dark:text-slate-200">
        <RouterLink
          to={`/artifacts/${link.targetUuid}`}
          className={clsx(
            "hover:underline",
            link.targetSummary
              ? "text-slate-800 dark:text-slate-200"
              : "text-slate-500",
          )}
        >
          <span className="font-mono text-slate-500">
            {link.hint.projectSlug}/{link.hint.collectionPrefix}/
          </span>
          {link.hint.artifactName}
        </RouterLink>
        <span className="ml-2 text-xs text-amber-700 dark:text-amber-300">
          unknown link type
        </span>
      </span>
    );
  }

  // unresolved
  return (
    <span className="text-slate-700 dark:text-slate-200">
      <span className="font-mono text-slate-500">
        {link.hint.projectSlug}/{link.hint.collectionPrefix}/
      </span>
      {link.hint.artifactName}
      <UnresolvedBadge hint={link.hint} />
    </span>
  );
}

function groupByType<T extends { link: LinkView }>(
  items: T[],
): Record<string, T[]> {
  const out: Record<string, T[]> = {};
  for (const item of items) {
    const key = item.link.type;
    if (!out[key]) out[key] = [];
    out[key].push(item);
  }
  return out;
}
