import { useState } from "react";
import clsx from "clsx";

import { useSubmitReview } from "../../api/queries";
import type {
  ArtifactDetail,
  CreateReviewRequest,
  ReviewState,
} from "../../api/types";
import {
  AddTodoDialog,
  ApproveDialog,
  ReRequestDialog,
  RejectDialog,
  ResolveTodoPopover,
} from "./ReviewActionDialogs";
import { SinceLastApprovalDiff } from "./SinceLastApprovalDiff";
import { SinceLastApprovalTimeline } from "./SinceLastApprovalTimeline";

const STATE_LABELS: Record<ReviewState, string> = {
  neverReviewed: "Never reviewed",
  approved: "Approved",
  rejected: "Rejected",
  reRequested: "Re-review requested",
};

const STATE_CLASSES: Record<ReviewState, string> = {
  neverReviewed:
    "bg-slate-200 text-slate-700 dark:bg-slate-700 dark:text-slate-200",
  approved:
    "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/50 dark:text-emerald-200",
  rejected: "bg-rose-100 text-rose-800 dark:bg-rose-900/50 dark:text-rose-200",
  reRequested:
    "bg-amber-100 text-amber-800 dark:bg-amber-900/50 dark:text-amber-100",
};

type ActiveDialog =
  | { kind: "none" }
  | { kind: "approve" }
  | { kind: "reject" }
  | { kind: "add-todo" }
  | { kind: "re-request" }
  | { kind: "resolve"; todoId: string };

export function ReviewPane({ artifact }: { artifact: ArtifactDetail }) {
  const mutation = useSubmitReview(artifact.uuid);
  const [active, setActive] = useState<ActiveDialog>({ kind: "none" });
  const close = () => setActive({ kind: "none" });
  const derived = artifact.reviewState;

  const submit = (req: CreateReviewRequest) => {
    mutation.mutate(req, { onSuccess: close });
  };

  return (
    <section aria-labelledby="review-pane-heading" className="space-y-3">
      <div className="flex items-center justify-between">
        <h2
          id="review-pane-heading"
          className="text-sm font-semibold tracking-wide text-slate-700 dark:text-slate-300"
        >
          Review state
        </h2>
        <span
          className={clsx(
            "rounded-full px-2 py-0.5 text-xs font-medium",
            STATE_CLASSES[derived.state],
          )}
          aria-label={`review state: ${STATE_LABELS[derived.state]}`}
        >
          {STATE_LABELS[derived.state]}
        </span>
      </div>

      {derived.state === "neverReviewed" ? (
        <p className="rounded border border-slate-200 bg-slate-50 p-2 text-xs text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300">
          No prior approval — this is the first review.
        </p>
      ) : null}

      <div className="flex flex-wrap gap-2">
        <ActionButton
          label="Approve"
          disabled={derived.blockingTodos.length > 0}
          onClick={() => setActive({ kind: "approve" })}
        />
        <ActionButton
          label="Reject with TODO"
          onClick={() => setActive({ kind: "reject" })}
        />
        <ActionButton
          label="Add TODO"
          onClick={() => setActive({ kind: "add-todo" })}
        />
        <ActionButton
          label="Re-request review"
          disabled={
            derived.state === "neverReviewed" && !derived.lastApprovalAt
          }
          onClick={() => setActive({ kind: "re-request" })}
        />
      </div>

      {derived.blockingTodos.length > 0 ? (
        <section aria-labelledby="blocking-todos-heading" className="space-y-1">
          <h3
            id="blocking-todos-heading"
            className="text-xs font-semibold uppercase tracking-wide text-slate-500"
          >
            Blocking TODOs
          </h3>
          <ul className="space-y-2 text-sm">
            {derived.blockingTodos.map((t) => (
              <li
                key={t.id}
                className="rounded border border-amber-300 bg-amber-50 p-2 dark:border-amber-800 dark:bg-amber-900/30"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-slate-800 dark:text-slate-100">
                    {t.text}
                  </span>
                  <button
                    type="button"
                    onClick={() => setActive({ kind: "resolve", todoId: t.id })}
                    className="text-xs text-slate-700 hover:underline dark:text-slate-300"
                  >
                    Resolve
                  </button>
                </div>
                <p className="text-xs text-slate-500">
                  added by {t.addedBy} · {t.addedAt}
                </p>
                {active.kind === "resolve" && active.todoId === t.id ? (
                  <ResolveTodoPopover
                    projectSlug={artifact.projectSlug}
                    todo={t}
                    onSubmit={submit}
                    onClose={close}
                    pending={mutation.isPending}
                    errorMessage={extractMutationError(mutation.error)}
                  />
                ) : null}
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <section aria-labelledby="review-log-heading" className="space-y-1">
        <h3
          id="review-log-heading"
          className="text-xs font-semibold uppercase tracking-wide text-slate-500"
        >
          Review log
        </h3>
        {artifact.reviewLog.length === 0 ? (
          <p className="text-sm text-slate-500">No review events yet.</p>
        ) : (
          <ol className="space-y-1 text-sm">
            {artifact.reviewLog.map((entry, i) => (
              <li key={`${entry.timestamp}:${i}`}>
                <span className="font-medium">{entry.outcome}</span>{" "}
                <span className="text-slate-500">by {entry.reviewer}</span>{" "}
                <span className="font-mono text-xs text-slate-500">
                  {entry.timestamp}
                </span>
                {entry.explanation ? (
                  <span className="ml-1 text-slate-600 dark:text-slate-400">
                    — {entry.explanation}
                  </span>
                ) : null}
              </li>
            ))}
          </ol>
        )}
      </section>

      {derived.lastApprovalAt ? (
        <details className="rounded border border-slate-200 dark:border-slate-700">
          <summary className="cursor-pointer p-2 text-sm font-medium text-slate-700 dark:text-slate-200">
            Since last approval
          </summary>
          <div className="space-y-3 p-2">
            <SinceLastApprovalDiff
              uuid={artifact.uuid}
              enabled={true}
              currentBody={artifact.body ?? ""}
              currentMetadata={{
                title: artifact.title,
                description: artifact.description,
                tags: artifact.tags,
                outlineLevel: artifact.outlineLevel,
                active: artifact.active,
                derived: artifact.derived,
              }}
            />
            <SinceLastApprovalTimeline
              log={artifact.reviewLog}
              lastApprovalAt={derived.lastApprovalAt}
              blockingTodos={derived.blockingTodos}
            />
          </div>
        </details>
      ) : null}

      {active.kind === "approve" ? (
        <ApproveDialog
          projectSlug={artifact.projectSlug}
          derived={derived}
          onSubmit={submit}
          onClose={close}
          pending={mutation.isPending}
          errorMessage={extractMutationError(mutation.error)}
        />
      ) : null}
      {active.kind === "reject" ? (
        <RejectDialog
          projectSlug={artifact.projectSlug}
          derived={derived}
          onSubmit={submit}
          onClose={close}
          pending={mutation.isPending}
          errorMessage={extractMutationError(mutation.error)}
        />
      ) : null}
      {active.kind === "add-todo" ? (
        <AddTodoDialog
          projectSlug={artifact.projectSlug}
          derived={derived}
          onSubmit={submit}
          onClose={close}
          pending={mutation.isPending}
          errorMessage={extractMutationError(mutation.error)}
        />
      ) : null}
      {active.kind === "re-request" ? (
        <ReRequestDialog
          projectSlug={artifact.projectSlug}
          derived={derived}
          onSubmit={submit}
          onClose={close}
          pending={mutation.isPending}
          errorMessage={extractMutationError(mutation.error)}
        />
      ) : null}
    </section>
  );
}

function ActionButton({
  label,
  onClick,
  disabled,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="rounded border border-slate-300 px-2 py-0.5 text-xs hover:bg-slate-50 disabled:opacity-50 dark:border-slate-600 dark:hover:bg-slate-800"
    >
      {label}
    </button>
  );
}

function extractMutationError(error: unknown): string | undefined {
  if (!error) return undefined;
  if (error instanceof Error) return error.message;
  return String(error);
}
