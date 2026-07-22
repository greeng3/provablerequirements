import type { Tone } from "../labels";

type Props = {
  label: string;
  tone: Tone;
};

const TONE: Record<Tone, string> = {
  accent: "text-accent border-accent/30 bg-accent/10",
  info: "text-info border-info/30 bg-info/10",
  warn: "text-warn border-warn/30 bg-warn/10",
  ok: "text-ok border-ok/30 bg-ok/10",
  muted: "text-muted border-border bg-surface-2",
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
