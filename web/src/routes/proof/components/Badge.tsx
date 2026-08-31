import type { Tone } from "../labels";

type Props = {
  label: string;
  tone: Tone;
};

/// A compact status pill in the management frontend's slate palette. The old
/// proof UI's oklch semantic tokens (ok/warn/info/accent/muted) are mapped to
/// the Tailwind families the rest of the app uses: warn→amber, ok→emerald,
/// info→sky, accent→indigo, muted→slate.
const TONE: Record<Tone, string> = {
  accent:
    "border-indigo-300 bg-indigo-50 text-indigo-700 dark:border-indigo-800 dark:bg-indigo-950/40 dark:text-indigo-300",
  info: "border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-800 dark:bg-sky-950/40 dark:text-sky-300",
  warn: "border-amber-300 bg-amber-50 text-amber-700 dark:border-amber-800 dark:bg-amber-950/40 dark:text-amber-300",
  ok: "border-emerald-300 bg-emerald-50 text-emerald-700 dark:border-emerald-800 dark:bg-emerald-950/40 dark:text-emerald-300",
  muted:
    "border-slate-200 bg-slate-100 text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400",
};

export function Badge({ label, tone }: Props) {
  return (
    <span
      className={`inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium ${TONE[tone]}`}
    >
      {label}
    </span>
  );
}
