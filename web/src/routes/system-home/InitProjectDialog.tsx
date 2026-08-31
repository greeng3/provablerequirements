import { useEffect, useId, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";

import { ApiError } from "../../api/client";
import { useCreateSampleContent, useInitProject } from "../../api/queries";

interface Props {
  readonly dirName: string;
  readonly onClose: () => void;
}

/// Phase 11b: the init wizard is now two-stage. Stage 1 is the
/// existing slug / name / description form. Stage 2 is a post-
/// init choice panel with three buttons: Start empty / Create
/// sample content / Import from doorstop (handled by the caller
/// via `onClose` + navigation). Matches UX-postInitChoice.
type Stage = { kind: "form" } | { kind: "done"; slug: string };

export function InitProjectDialog({ dirName, onClose }: Props) {
  const navigate = useNavigate();
  const initMutation = useInitProject(dirName);
  const slugId = useId();
  const nameId = useId();
  const [slug, setSlug] = useState(dirName);
  const [name, setName] = useState(dirName);
  const [description, setDescription] = useState("");
  const [stage, setStage] = useState<Stage>({ kind: "form" });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const trimmedSlug = slug.trim();
    initMutation.mutate(
      {
        slug: trimmedSlug,
        name: name.trim(),
        description: description.trim() === "" ? null : description.trim(),
      },
      {
        onSuccess: () => setStage({ kind: "done", slug: trimmedSlug }),
      },
    );
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="init-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="init-heading" className="text-lg font-semibold tracking-tight">
          {stage.kind === "form"
            ? `Initialise ${dirName} as a ReqForge Project`
            : `${dirName} is ready — what next?`}
        </h2>
        {stage.kind === "form" ? (
          <FormStage
            slug={slug}
            name={name}
            description={description}
            slugId={slugId}
            nameId={nameId}
            isPending={initMutation.isPending}
            error={initMutation.error}
            onSlugChange={setSlug}
            onNameChange={setName}
            onDescriptionChange={setDescription}
            onSubmit={submit}
            onCancel={onClose}
          />
        ) : (
          <ChoiceStage
            slug={stage.slug}
            onDone={() => {
              onClose();
              navigate(`/projects/${stage.slug}`);
            }}
          />
        )}
      </div>
    </div>
  );
}

interface FormStageProps {
  readonly slug: string;
  readonly name: string;
  readonly description: string;
  readonly slugId: string;
  readonly nameId: string;
  readonly isPending: boolean;
  readonly error: Error | null;
  readonly onSlugChange: (value: string) => void;
  readonly onNameChange: (value: string) => void;
  readonly onDescriptionChange: (value: string) => void;
  readonly onSubmit: (e: FormEvent<HTMLFormElement>) => void;
  readonly onCancel: () => void;
}

function FormStage({
  slug,
  name,
  description,
  slugId,
  nameId,
  isPending,
  error,
  onSlugChange,
  onNameChange,
  onDescriptionChange,
  onSubmit,
  onCancel,
}: FormStageProps) {
  return (
    <>
      <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
        Writes a reqforge.json at the mount root and creates the default{" "}
        <span className="font-mono">artifacts/</span> directory. Safe to run on
        a fresh git repository.
      </p>
      <form onSubmit={onSubmit} className="mt-4 space-y-3">
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            Slug
          </span>
          <input
            id={slugId}
            required
            value={slug}
            onChange={(e) => onSlugChange(e.target.value)}
            pattern="[A-Za-z0-9._\-]+"
            className="w-full rounded border border-slate-300 px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
          />
          <span className="mt-1 block text-xs text-slate-500">
            Stable identifier within the System.
          </span>
        </label>
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            Display name
          </span>
          <input
            id={nameId}
            required
            value={name}
            onChange={(e) => onNameChange(e.target.value)}
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            Description (optional)
          </span>
          <input
            value={description}
            onChange={(e) => onDescriptionChange(e.target.value)}
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        {error ? (
          <p className="text-sm text-rose-600" role="alert">
            {String(error)}
          </p>
        ) : null}
        <div className="flex justify-end gap-2 pt-2">
          <button
            type="button"
            onClick={onCancel}
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={isPending || !slug || !name}
            className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
          >
            {isPending ? "Initialising…" : "Initialise"}
          </button>
        </div>
      </form>
    </>
  );
}

interface ChoiceStageProps {
  readonly slug: string;
  readonly onDone: () => void;
}

function ChoiceStage({ slug, onDone }: ChoiceStageProps) {
  const sample = useCreateSampleContent(slug);
  const [sampleError, setSampleError] = useState<string | null>(null);
  const runSample = () => {
    setSampleError(null);
    sample.mutate(undefined, {
      onSuccess: () => onDone(),
      onError: (err) => {
        if (err instanceof ApiError && err.status === 409) {
          const body = err.body as { error?: string } | undefined;
          setSampleError(body?.error ?? "Project is no longer empty.");
        } else {
          setSampleError(String(err));
        }
      },
    });
  };

  return (
    <div className="mt-4 space-y-3" data-testid="init-post-choice">
      <p className="text-sm text-slate-600 dark:text-slate-400">
        Pick how you'd like to start <span className="font-mono">{slug}</span>:
      </p>
      <ul className="space-y-2">
        <li>
          <button
            type="button"
            onClick={onDone}
            data-testid="init-choice-empty"
            className="flex w-full items-start gap-3 rounded border border-slate-200 p-3 text-left text-sm hover:border-slate-400 dark:border-slate-700 dark:hover:border-slate-500"
          >
            <span className="shrink-0 text-base leading-none">📄</span>
            <span>
              <strong className="block">Start empty</strong>
              <span className="text-xs text-slate-600 dark:text-slate-400">
                Go straight to the project page and build it up yourself.
              </span>
            </span>
          </button>
        </li>
        <li>
          <button
            type="button"
            onClick={runSample}
            disabled={sample.isPending}
            data-testid="init-choice-sample"
            className="flex w-full items-start gap-3 rounded border border-slate-200 p-3 text-left text-sm hover:border-slate-400 disabled:opacity-50 dark:border-slate-700 dark:hover:border-slate-500"
          >
            <span className="shrink-0 text-base leading-none">🧩</span>
            <span>
              <strong className="block">
                {sample.isPending ? "Seeding…" : "Create sample content"}
              </strong>
              <span className="text-xs text-slate-600 dark:text-slate-400">
                Seed the project with a small Task Tracker demo — three
                collections, seven artifacts, linked end-to-end.
              </span>
            </span>
          </button>
        </li>
        <li>
          <button
            type="button"
            onClick={onDone}
            data-testid="init-choice-doorstop"
            className="flex w-full items-start gap-3 rounded border border-slate-200 p-3 text-left text-sm hover:border-slate-400 dark:border-slate-700 dark:hover:border-slate-500"
          >
            <span className="shrink-0 text-base leading-none">📥</span>
            <span>
              <strong className="block">Import from doorstop</strong>
              <span className="text-xs text-slate-600 dark:text-slate-400">
                Go to the project page and launch the doorstop import wizard
                yourself.
              </span>
            </span>
          </button>
        </li>
      </ul>
      {sampleError ? (
        <p
          className="text-sm text-rose-600"
          role="alert"
          data-testid="init-choice-error"
        >
          {sampleError}
        </p>
      ) : null}
    </div>
  );
}
