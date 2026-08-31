import { useState } from "react";

import {
  useAcknowledgeLlmPrivacy,
  useAddLlmProvider,
  useDeleteLlmProvider,
  useLlmProviders,
  usePatchLlmProvider,
  useReplaceLlmProvider,
  useRetestLlmProvider,
} from "../../api/queries";
import type { LlmHealthState, LlmProviderEntry } from "../../api/types";

type ProviderFamily = "openai-compatible" | "anthropic" | "gemini";

/// Phase 13 LLM-providers page at `/llm`. Lists every configured
/// provider with health, key state, and Retest / Acknowledge /
/// Enable / Edit / Delete controls. Add-provider form sits at
/// the top so operators can configure new entries without
/// hand-editing `system.json`.
export function LlmProvidersPage() {
  const providers = useLlmProviders();
  const [showAddForm, setShowAddForm] = useState(false);

  return (
    <section aria-labelledby="llm-heading" className="space-y-4">
      <header className="flex flex-wrap items-baseline justify-between gap-3">
        <div>
          <h1
            id="llm-heading"
            className="text-2xl font-semibold tracking-tight"
          >
            LLM providers
          </h1>
          <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
            ReqForge walks this list top-to-bottom when a feature needs an LLM,
            skipping disabled entries. On failure it falls through; permanent
            failures (auth, unknown model) hard-disable the slot for this
            process until a retest.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setShowAddForm((v) => !v)}
          data-testid="llm-add-toggle"
          className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 dark:bg-slate-100 dark:text-slate-900"
        >
          {showAddForm ? "Cancel" : "Add provider"}
        </button>
      </header>

      {showAddForm ? (
        <ProviderForm mode="add" onClose={() => setShowAddForm(false)} />
      ) : null}

      {providers.isLoading ? (
        <p className="text-sm text-slate-500">Loading…</p>
      ) : providers.isError ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load provider list: {String(providers.error)}
        </p>
      ) : !providers.data || providers.data.providers.length === 0 ? (
        <p data-testid="llm-providers-empty" className="text-sm text-slate-500">
          No LLM providers configured. Click <strong>Add provider</strong> above
          to register one — entries persist to{" "}
          <span className="font-mono">system.json</span> via the UI.
        </p>
      ) : (
        <ul className="space-y-3">
          {providers.data.providers.map((entry) => (
            <ProviderRow key={entry.index} entry={entry} />
          ))}
        </ul>
      )}
    </section>
  );
}

/// Shared form for add + edit. The two modes differ only in
/// initial values and which mutation runs on submit.
interface ProviderFormProps {
  readonly mode: "add" | "edit";
  readonly editIndex?: number;
  readonly initial?: {
    provider: ProviderFamily;
    model: string;
    endpoint: string;
  };
  readonly onClose: () => void;
}

function ProviderForm({
  mode,
  editIndex,
  initial,
  onClose,
}: ProviderFormProps) {
  const add = useAddLlmProvider();
  const replace = useReplaceLlmProvider();
  const [provider, setProvider] = useState<ProviderFamily>(
    initial?.provider ?? "openai-compatible",
  );
  const [model, setModel] = useState(initial?.model ?? "");
  const [endpoint, setEndpoint] = useState(initial?.endpoint ?? "");
  const [apiKey, setApiKey] = useState("");
  const [keepExistingKey, setKeepExistingKey] = useState(mode === "edit");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const pending = add.isPending || replace.isPending;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMsg(null);
    if (!model.trim()) {
      setErrorMsg("Model is required.");
      return;
    }
    if (provider === "openai-compatible" && !endpoint.trim()) {
      setErrorMsg("Endpoint is required for openai-compatible providers.");
      return;
    }
    const apiKeyForRequest =
      mode === "edit" && keepExistingKey ? undefined : apiKey ? apiKey : "";
    const req = {
      provider,
      model: model.trim(),
      endpoint: endpoint.trim() || undefined,
      // For edit: undefined = keep, "" = clear, value = replace.
      // For add: undefined = unset, value = set.
      apiKey:
        mode === "edit" && keepExistingKey
          ? undefined
          : apiKey
            ? apiKey
            : undefined,
    };
    void apiKeyForRequest; // future hook for explicit-clear UX
    const onSuccess = () => {
      setModel("");
      setEndpoint("");
      setApiKey("");
      onClose();
    };
    const onError = (err: unknown) => setErrorMsg(String(err));
    if (mode === "edit" && typeof editIndex === "number") {
      replace.mutate({ index: editIndex, req }, { onSuccess, onError });
    } else {
      add.mutate(req, { onSuccess, onError });
    }
  };

  return (
    <form
      onSubmit={submit}
      data-testid={mode === "edit" ? "llm-edit-form" : "llm-add-form"}
      className="rounded border border-slate-300 bg-white p-4 dark:border-slate-700 dark:bg-slate-900"
    >
      <h2 className="text-base font-semibold">
        {mode === "edit" ? "Edit LLM provider" : "Add LLM provider"}
      </h2>
      <p className="mt-1 text-xs text-slate-500">
        Fields persist to <span className="font-mono">system.json</span> at mode
        0600. Leave the API key blank for keyless providers (e.g. a local Ollama
        instance).
      </p>

      <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
        <label className="text-sm">
          <span className="block font-medium text-slate-700 dark:text-slate-300">
            Provider
          </span>
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value as ProviderFamily)}
            data-testid="llm-form-provider"
            className="mt-1 w-full rounded border border-slate-300 bg-white px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          >
            <option value="openai-compatible">openai-compatible</option>
            <option value="anthropic">anthropic</option>
            <option value="gemini">gemini</option>
          </select>
        </label>
        <label className="text-sm">
          <span className="block font-medium text-slate-700 dark:text-slate-300">
            Model
          </span>
          <input
            type="text"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={
              provider === "openai-compatible"
                ? "qwen2.5-coder:14b"
                : provider === "anthropic"
                  ? "claude-haiku-4-5"
                  : "gemini-2.0-flash"
            }
            data-testid="llm-form-model"
            className="mt-1 w-full rounded border border-slate-300 bg-white px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <label className="text-sm sm:col-span-2">
          <span className="block font-medium text-slate-700 dark:text-slate-300">
            Endpoint{" "}
            {provider !== "openai-compatible" ? (
              <span className="text-slate-500">(optional)</span>
            ) : (
              <span className="text-rose-600">(required)</span>
            )}
          </span>
          <input
            type="text"
            value={endpoint}
            onChange={(e) => setEndpoint(e.target.value)}
            placeholder={
              provider === "openai-compatible"
                ? "http://host.docker.internal:11434"
                : "(uses provider default)"
            }
            data-testid="llm-form-endpoint"
            className="mt-1 w-full rounded border border-slate-300 bg-white px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
          />
        </label>
        <label className="text-sm sm:col-span-2">
          <span className="block font-medium text-slate-700 dark:text-slate-300">
            API key{" "}
            <span className="text-slate-500">
              (leave blank for keyless providers)
            </span>
          </span>
          {mode === "edit" && keepExistingKey ? (
            <div className="mt-1 flex items-center gap-2 text-sm">
              <span className="rounded bg-slate-100 px-2 py-1 text-xs dark:bg-slate-800">
                key on file unchanged
              </span>
              <button
                type="button"
                onClick={() => setKeepExistingKey(false)}
                data-testid="llm-form-replace-key"
                className="text-xs text-slate-500 hover:underline"
              >
                Replace key
              </button>
            </div>
          ) : (
            <input
              type="password"
              autoComplete="off"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              data-testid="llm-form-api-key"
              className="mt-1 w-full rounded border border-slate-300 bg-white px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
            />
          )}
        </label>
      </div>

      {errorMsg ? (
        <p
          className="mt-3 text-sm text-rose-600"
          role="alert"
          data-testid="llm-form-error"
        >
          {errorMsg}
        </p>
      ) : null}

      <div className="mt-4 flex justify-end gap-2">
        <button
          type="button"
          onClick={onClose}
          className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={pending}
          data-testid="llm-form-submit"
          className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
        >
          {pending
            ? mode === "edit"
              ? "Saving…"
              : "Adding…"
            : mode === "edit"
              ? "Save changes"
              : "Add provider"}
        </button>
      </div>
    </form>
  );
}

interface ProviderRowProps {
  readonly entry: LlmProviderEntry;
}

function ProviderRow({ entry }: ProviderRowProps) {
  const retest = useRetestLlmProvider();
  const ack = useAcknowledgeLlmPrivacy();
  const del = useDeleteLlmProvider();
  const patch = usePatchLlmProvider();
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [editing, setEditing] = useState(false);

  if (editing) {
    return (
      <li
        data-testid={`llm-provider-${entry.index}`}
        className="rounded border border-slate-200 p-2 dark:border-slate-800"
      >
        <ProviderForm
          mode="edit"
          editIndex={entry.index}
          initial={{
            provider: entry.provider as ProviderFamily,
            model: entry.model,
            endpoint: entry.endpoint,
          }}
          onClose={() => setEditing(false)}
        />
      </li>
    );
  }

  return (
    <li
      data-testid={`llm-provider-${entry.index}`}
      className={
        entry.enabled
          ? "rounded border border-slate-200 p-4 dark:border-slate-800"
          : "rounded border border-slate-200 bg-slate-50 p-4 opacity-75 dark:border-slate-800 dark:bg-slate-900/50"
      }
    >
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-base font-semibold">
            <span className="font-mono">{entry.provider}</span>
            <span className="mx-2 text-slate-400">/</span>
            <span className="font-mono">{entry.model}</span>
          </h2>
          <p className="mt-1 text-xs text-slate-500 break-all">
            {entry.endpoint}
            {entry.isLocal ? (
              <span className="ml-2 rounded bg-slate-100 px-1.5 text-[10px] uppercase tracking-wide dark:bg-slate-800">
                local
              </span>
            ) : null}
            {!entry.enabled ? (
              <span className="ml-2 rounded bg-slate-200 px-1.5 text-[10px] uppercase tracking-wide dark:bg-slate-700">
                disabled
              </span>
            ) : null}
          </p>
        </div>
        <HealthBadge health={entry.health} />
      </div>

      <dl className="mt-3 grid grid-cols-1 gap-2 text-xs sm:grid-cols-2">
        <div>
          <dt className="font-medium text-slate-500">API key</dt>
          <dd>
            {entry.apiKeyAvailable ? (
              <span className="text-emerald-700 dark:text-emerald-300">
                configured
              </span>
            ) : (
              <span className="text-amber-700 dark:text-amber-300">
                not configured
              </span>
            )}
          </dd>
        </div>
        <div>
          <dt className="font-medium text-slate-500">Privacy</dt>
          <dd>
            {entry.isLocal ? (
              <span className="text-slate-500">
                local endpoint, no warning needed
              </span>
            ) : entry.requiresPrivacyAck ? (
              <span className="text-amber-700 dark:text-amber-300">
                warning not yet acknowledged
              </span>
            ) : (
              <span className="text-emerald-700 dark:text-emerald-300">
                acknowledged
              </span>
            )}
          </dd>
        </div>
      </dl>

      <div className="mt-3 flex flex-wrap gap-2">
        <label className="flex items-center gap-1 text-xs">
          <input
            type="checkbox"
            checked={entry.enabled}
            onChange={(e) =>
              patch.mutate({
                index: entry.index,
                req: { enabled: e.target.checked },
              })
            }
            disabled={patch.isPending}
            data-testid={`llm-enabled-${entry.index}`}
          />
          Enabled
        </label>
        <button
          type="button"
          onClick={() => setEditing(true)}
          data-testid={`llm-edit-${entry.index}`}
          className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          Edit
        </button>
        <button
          type="button"
          onClick={() => retest.mutate(entry.index)}
          disabled={retest.isPending}
          data-testid={`llm-retest-${entry.index}`}
          className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 disabled:opacity-50 dark:border-slate-600 dark:hover:bg-slate-800"
        >
          {retest.isPending ? "Retesting…" : "Retest"}
        </button>
        {entry.requiresPrivacyAck ? (
          <button
            type="button"
            onClick={() => ack.mutate(entry.index)}
            disabled={ack.isPending}
            data-testid={`llm-ack-${entry.index}`}
            className="rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
          >
            {ack.isPending ? "Acknowledging…" : "Acknowledge privacy"}
          </button>
        ) : null}
        {confirmingDelete ? (
          <>
            <span className="text-xs text-rose-700 dark:text-rose-400">
              Delete this provider?
            </span>
            <button
              type="button"
              onClick={() => del.mutate(entry.index)}
              disabled={del.isPending}
              data-testid={`llm-delete-confirm-${entry.index}`}
              className="rounded bg-rose-600 px-3 py-1 text-xs font-semibold text-white hover:bg-rose-700 disabled:opacity-50"
            >
              {del.isPending ? "Deleting…" : "Yes, delete"}
            </button>
            <button
              type="button"
              onClick={() => setConfirmingDelete(false)}
              className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
            >
              Cancel
            </button>
          </>
        ) : (
          <button
            type="button"
            onClick={() => setConfirmingDelete(true)}
            data-testid={`llm-delete-${entry.index}`}
            className="rounded border border-rose-400 px-3 py-1 text-xs text-rose-700 hover:bg-rose-50 dark:border-rose-700 dark:text-rose-400 dark:hover:bg-rose-950"
          >
            Delete
          </button>
        )}
      </div>
      {retest.error ? (
        <p className="mt-2 text-xs text-rose-600" role="alert">
          Retest failed: {String(retest.error)}
        </p>
      ) : null}
      {retest.data && !retest.data.ok ? (
        <p className="mt-2 text-xs text-rose-600" role="alert">
          Retest reported an error: {retest.data.error}
        </p>
      ) : null}
      {del.error ? (
        <p className="mt-2 text-xs text-rose-600" role="alert">
          Delete failed: {String(del.error)}
        </p>
      ) : null}
      {patch.error ? (
        <p className="mt-2 text-xs text-rose-600" role="alert">
          Update failed: {String(patch.error)}
        </p>
      ) : null}
    </li>
  );
}

function HealthBadge({ health }: { readonly health: LlmHealthState }) {
  if (health.kind === "healthy") {
    return (
      <span className="rounded bg-emerald-100 px-2 py-0.5 text-xs font-medium text-emerald-900 dark:bg-emerald-900/40 dark:text-emerald-100">
        healthy
      </span>
    );
  }
  if (health.kind === "transient-degraded") {
    return (
      <span
        title={`retry in ~${health.retryAfterSecs}s`}
        className="rounded bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-900 dark:bg-amber-900/40 dark:text-amber-100"
      >
        transient-degraded
      </span>
    );
  }
  return (
    <span className="rounded bg-rose-100 px-2 py-0.5 text-xs font-medium text-rose-900 dark:bg-rose-900/40 dark:text-rose-100">
      hard-disabled
    </span>
  );
}
