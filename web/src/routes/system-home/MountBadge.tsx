import clsx from "clsx";
import type { MountStateTag } from "../../api/types";

const BADGE_LABELS: Record<MountStateTag, string> = {
  project: "Project",
  needsInit: "Needs init",
  noGit: "No git",
  loadFailed: "Load failed",
};

const BADGE_CLASSES: Record<MountStateTag, string> = {
  project:
    "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/50 dark:text-emerald-200",
  needsInit:
    "bg-amber-100 text-amber-800 dark:bg-amber-900/50 dark:text-amber-200",
  noGit: "bg-slate-200 text-slate-700 dark:bg-slate-700 dark:text-slate-200",
  loadFailed:
    "bg-rose-100 text-rose-800 dark:bg-rose-900/50 dark:text-rose-200",
};

export function MountBadge({ state }: { state: MountStateTag }) {
  return (
    <span
      className={clsx(
        "inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium",
        BADGE_CLASSES[state],
      )}
      aria-label={`mount state: ${BADGE_LABELS[state]}`}
    >
      {BADGE_LABELS[state]}
    </span>
  );
}
