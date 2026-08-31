import { useEffect } from "react";

interface Props {
  readonly open: boolean;
  readonly onClose: () => void;
}

/// Phase 11c: documents the keyboard shortcut set per
/// `UX-keyboardShortcuts`. ReqForge ships the CodeMirror default
/// set + a global save shortcut + the standard Escape-closes-
/// dialog convention. This modal lists them for discoverability.
/// Triggered by the header help button and by the global `?`
/// hotkey wired up in `AppShell`.
export function KeyboardShortcutsOverlay({ open, onClose }: Props) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="keyboard-shortcuts-heading"
      data-testid="keyboard-shortcuts-overlay"
      className="fixed inset-0 z-20 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="keyboard-shortcuts-heading"
          className="text-lg font-semibold tracking-tight"
        >
          Keyboard shortcuts
        </h2>
        <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
          ReqForge ships only the shortcuts you'd reasonably expect from a
          browser-based Markdown editor. Navigation shortcuts are deferred until
          a concrete request specifies them.
        </p>

        <Section title="Everywhere">
          <Shortcut keys={["?"]} label="Open this help" />
          <Shortcut keys={["Esc"]} label="Close any open dialog" />
        </Section>

        <Section title="Markdown editor (CodeMirror defaults)">
          <Shortcut
            keys={["Ctrl", "S"]}
            label="Save the artifact"
            alt={["⌘", "S"]}
          />
          <Shortcut
            keys={["Ctrl", "B"]}
            label="Bold selection"
            alt={["⌘", "B"]}
          />
          <Shortcut
            keys={["Ctrl", "I"]}
            label="Italic selection"
            alt={["⌘", "I"]}
          />
          <Shortcut keys={["Ctrl", "Z"]} label="Undo" alt={["⌘", "Z"]} />
          <Shortcut
            keys={["Ctrl", "Shift", "Z"]}
            label="Redo"
            alt={["⌘", "Shift", "Z"]}
          />
          <Shortcut
            keys={["Ctrl", "F"]}
            label="Find in editor"
            alt={["⌘", "F"]}
          />
        </Section>

        <div className="mt-6 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 dark:bg-slate-100 dark:text-slate-900"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

interface SectionProps {
  readonly title: string;
  readonly children: React.ReactNode;
}

function Section({ title, children }: SectionProps) {
  return (
    <div className="mt-4">
      <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500">
        {title}
      </h3>
      <dl className="mt-2 space-y-1 text-sm">{children}</dl>
    </div>
  );
}

interface ShortcutProps {
  readonly keys: string[];
  readonly label: string;
  readonly alt?: string[];
}

function Shortcut({ keys, label, alt }: ShortcutProps) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <dt>{label}</dt>
      <dd className="flex items-center gap-1 font-mono text-xs">
        <Keys keys={keys} />
        {alt ? (
          <>
            <span className="px-1 text-slate-400">/</span>
            <Keys keys={alt} />
          </>
        ) : null}
      </dd>
    </div>
  );
}

function Keys({ keys }: { readonly keys: string[] }) {
  return (
    <span className="flex items-center gap-0.5">
      {keys.map((k, i) => (
        <span
          key={`${k}-${i}`}
          className="rounded border border-slate-300 bg-slate-100 px-1.5 py-0.5 text-[10px] dark:border-slate-600 dark:bg-slate-800"
        >
          {k}
        </span>
      ))}
    </span>
  );
}
