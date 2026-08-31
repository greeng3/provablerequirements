import { useEffect, useState, type FormEvent } from "react";

import type {
  CreateReviewRequest,
  DerivedReviewState,
  OpenTodo,
} from "../../api/types";
import { ReviewerSelect } from "./ReviewerSelect";

/// Shared modal shell — every action dialog uses the same backdrop
/// + Escape-to-close wiring, so I factor it out rather than copying.
function DialogShell({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="fixed inset-0 z-20 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold tracking-tight">{title}</h2>
        <div className="mt-4">{children}</div>
      </div>
    </div>
  );
}

interface ActionProps {
  projectSlug: string;
  derived: DerivedReviewState;
  onSubmit: (req: CreateReviewRequest) => void;
  onClose: () => void;
  pending?: boolean;
  errorMessage?: string;
}

function Footer({
  onClose,
  disabled,
  pending,
  label,
}: {
  onClose: () => void;
  disabled?: boolean;
  pending?: boolean;
  label: string;
}) {
  return (
    <div className="mt-5 flex justify-end gap-2">
      <button
        type="button"
        onClick={onClose}
        className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
      >
        Cancel
      </button>
      <button
        type="submit"
        disabled={disabled || pending}
        className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
      >
        {pending ? "Submitting…" : label}
      </button>
    </div>
  );
}

function ReviewerField({
  projectSlug,
  reviewer,
  setReviewer,
}: {
  projectSlug: string;
  reviewer: string;
  setReviewer: (next: string) => void;
}) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
        Reviewer
      </span>
      <ReviewerSelect
        projectSlug={projectSlug}
        value={reviewer}
        onChange={setReviewer}
      />
    </label>
  );
}

function ExplanationField({
  value,
  onChange,
}: {
  value: string;
  onChange: (next: string) => void;
}) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
        Explanation (optional)
      </span>
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        rows={3}
        className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
      />
    </label>
  );
}

function ErrorMessage({ message }: { message?: string }) {
  if (!message) return null;
  return (
    <p className="text-sm text-rose-600" role="alert">
      {message}
    </p>
  );
}

export function ApproveDialog(props: ActionProps) {
  const { projectSlug, derived, onSubmit, onClose, pending, errorMessage } =
    props;
  const [reviewer, setReviewer] = useState("");
  const [explanation, setExplanation] = useState("");

  const blocked = derived.blockingTodos.length > 0;

  const submit = (e: FormEvent) => {
    e.preventDefault();
    onSubmit({
      reviewer,
      action: "approve",
      explanation: explanation.trim() === "" ? undefined : explanation,
    });
  };

  return (
    <DialogShell title="Approve review" onClose={onClose}>
      <form onSubmit={submit} className="space-y-3">
        <ReviewerField
          projectSlug={projectSlug}
          reviewer={reviewer}
          setReviewer={setReviewer}
        />
        <ExplanationField value={explanation} onChange={setExplanation} />
        {blocked && (
          <p
            className="text-sm text-amber-700 dark:text-amber-300"
            role="alert"
          >
            Cannot approve while blocking TODOs are open. Resolve them first or
            reject with a new TODO instead.
          </p>
        )}
        <ErrorMessage message={errorMessage} />
        <Footer
          onClose={onClose}
          disabled={reviewer.trim() === "" || blocked}
          pending={pending}
          label="Approve"
        />
      </form>
    </DialogShell>
  );
}

export function RejectDialog(props: ActionProps) {
  const { projectSlug, onSubmit, onClose, pending, errorMessage } = props;
  const [reviewer, setReviewer] = useState("");
  const [todoText, setTodoText] = useState("");
  const [explanation, setExplanation] = useState("");

  const submit = (e: FormEvent) => {
    e.preventDefault();
    onSubmit({
      reviewer,
      action: "reject-with-todo",
      todo: { text: todoText },
      explanation: explanation.trim() === "" ? undefined : explanation,
    });
  };

  return (
    <DialogShell title="Reject with TODO" onClose={onClose}>
      <form onSubmit={submit} className="space-y-3">
        <ReviewerField
          projectSlug={projectSlug}
          reviewer={reviewer}
          setReviewer={setReviewer}
        />
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            Blocking TODO
          </span>
          <input
            type="text"
            value={todoText}
            onChange={(e) => setTodoText(e.target.value)}
            placeholder="What needs to happen before re-approval"
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <ExplanationField value={explanation} onChange={setExplanation} />
        <ErrorMessage message={errorMessage} />
        <Footer
          onClose={onClose}
          disabled={reviewer.trim() === "" || todoText.trim() === ""}
          pending={pending}
          label="Reject"
        />
      </form>
    </DialogShell>
  );
}

export function AddTodoDialog(props: ActionProps) {
  const { projectSlug, onSubmit, onClose, pending, errorMessage } = props;
  const [reviewer, setReviewer] = useState("");
  const [todoText, setTodoText] = useState("");
  const [explanation, setExplanation] = useState("");

  const submit = (e: FormEvent) => {
    e.preventDefault();
    onSubmit({
      reviewer,
      action: "add-todo",
      todo: { text: todoText },
      explanation: explanation.trim() === "" ? undefined : explanation,
    });
  };

  return (
    <DialogShell title="Add blocking TODO" onClose={onClose}>
      <form onSubmit={submit} className="space-y-3">
        <ReviewerField
          projectSlug={projectSlug}
          reviewer={reviewer}
          setReviewer={setReviewer}
        />
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            TODO
          </span>
          <input
            type="text"
            value={todoText}
            onChange={(e) => setTodoText(e.target.value)}
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <ExplanationField value={explanation} onChange={setExplanation} />
        <ErrorMessage message={errorMessage} />
        <Footer
          onClose={onClose}
          disabled={reviewer.trim() === "" || todoText.trim() === ""}
          pending={pending}
          label="Add TODO"
        />
      </form>
    </DialogShell>
  );
}

export function ReRequestDialog(props: ActionProps) {
  const { projectSlug, onSubmit, onClose, pending, errorMessage } = props;
  const [reviewer, setReviewer] = useState("");
  const [explanation, setExplanation] = useState("");

  const submit = (e: FormEvent) => {
    e.preventDefault();
    onSubmit({
      reviewer,
      action: "re-request-review",
      explanation: explanation.trim() === "" ? undefined : explanation,
    });
  };

  return (
    <DialogShell title="Re-request review" onClose={onClose}>
      <form onSubmit={submit} className="space-y-3">
        <ReviewerField
          projectSlug={projectSlug}
          reviewer={reviewer}
          setReviewer={setReviewer}
        />
        <ExplanationField value={explanation} onChange={setExplanation} />
        <ErrorMessage message={errorMessage} />
        <Footer
          onClose={onClose}
          disabled={reviewer.trim() === ""}
          pending={pending}
          label="Re-request"
        />
      </form>
    </DialogShell>
  );
}

/// Inline resolve-TODO action: a compact popover over a specific
/// blocking TODO row. Not a modal — the rest of the pane stays
/// interactive while the reviewer confirms.
export function ResolveTodoPopover({
  projectSlug,
  todo,
  onSubmit,
  onClose,
  pending,
  errorMessage,
}: {
  projectSlug: string;
  todo: OpenTodo;
  onSubmit: (req: CreateReviewRequest) => void;
  onClose: () => void;
  pending?: boolean;
  errorMessage?: string;
}) {
  const [reviewer, setReviewer] = useState("");

  const submit = (e: FormEvent) => {
    e.preventDefault();
    onSubmit({
      reviewer,
      action: "resolve-todo",
      todoId: todo.id,
    });
  };

  return (
    <form
      onSubmit={submit}
      className="mt-2 flex items-center gap-2 rounded border border-slate-200 bg-slate-50 p-2 text-sm dark:border-slate-700 dark:bg-slate-800"
    >
      <ReviewerSelect
        projectSlug={projectSlug}
        value={reviewer}
        onChange={setReviewer}
      />
      <button
        type="submit"
        disabled={reviewer.trim() === "" || pending}
        className="rounded bg-slate-900 px-2 py-1 text-xs text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
      >
        Resolve
      </button>
      <button
        type="button"
        onClick={onClose}
        className="text-xs text-slate-500 hover:underline"
      >
        Cancel
      </button>
      {errorMessage && (
        <p className="text-xs text-rose-600" role="alert">
          {errorMessage}
        </p>
      )}
    </form>
  );
}
