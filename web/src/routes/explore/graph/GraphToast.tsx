import { useEffect } from "react";

interface Props {
  readonly message: string;
  readonly tone?: "info" | "error";
  readonly onDismiss: () => void;
}

/// Minimal auto-dismissing toast for the graph canvas. Self-link
/// rejection and authoring success both flow through this rather
/// than dragging in a general-purpose toast framework — the
/// surface area is small enough to own inline.
export function GraphToast({ message, tone = "info", onDismiss }: Props) {
  useEffect(() => {
    const id = window.setTimeout(onDismiss, 4000);
    return () => window.clearTimeout(id);
  }, [onDismiss, message]);

  const palette =
    tone === "error"
      ? "border-rose-300 bg-rose-50 text-rose-900 dark:border-rose-700 dark:bg-rose-900/40 dark:text-rose-100"
      : "border-sky-300 bg-sky-50 text-sky-900 dark:border-sky-700 dark:bg-sky-900/40 dark:text-sky-100";

  return (
    <div
      role="status"
      aria-live="polite"
      data-testid="graph-toast"
      className={`fixed bottom-4 right-4 z-20 max-w-sm rounded border px-3 py-2 text-sm shadow ${palette}`}
    >
      <div className="flex items-start gap-3">
        <span className="flex-1">{message}</span>
        <button
          type="button"
          onClick={onDismiss}
          className="text-xs underline hover:no-underline"
        >
          dismiss
        </button>
      </div>
    </div>
  );
}
