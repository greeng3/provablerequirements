import clsx from "clsx";
import type { LinkType } from "../../api/types";

/// Small chip showing a link type's forward name. When full
/// metadata is supplied, it colours built-in vs. System-declared
/// entries differently and exposes directedness + inverse name as
/// a tooltip (`title` attribute, readable by both mouse and AT
/// without new Radix wiring).
export function LinkTypeBadge({
  typeName,
  metadata,
}: {
  typeName: string;
  metadata?: LinkType;
}) {
  const style = metadata
    ? metadata.source === "builtin"
      ? "bg-slate-200 text-slate-800 dark:bg-slate-700 dark:text-slate-100"
      : "bg-indigo-100 text-indigo-800 dark:bg-indigo-900/50 dark:text-indigo-200"
    : "bg-amber-100 text-amber-900 dark:bg-amber-900/50 dark:text-amber-100";

  const title = metadata
    ? describe(metadata)
    : `Unknown link type — not in the effective catalog`;

  return (
    <span
      className={clsx(
        "inline-flex items-center rounded px-1.5 py-0.5 text-xs font-mono",
        style,
      )}
      title={title}
      aria-label={`link type ${typeName}${metadata ? "" : " (unknown)"}`}
    >
      {typeName}
    </span>
  );
}

function describe(t: LinkType): string {
  const direction = t.directed ? "directed" : "symmetric";
  const cycles = t.acyclic ? ", acyclic" : "";
  return `${direction}${cycles} · inverse: ${t.inverseName} · ${t.source}`;
}
