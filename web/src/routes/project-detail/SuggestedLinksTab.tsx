import { useState } from "react";

import {
  useAcceptLinkSuggestion,
  useAnalyzeLinkSuggestions,
  useDeclinedLinkSuggestions,
  useLlmProviders,
  usePendingLinkSuggestions,
  useReinstateLinkSuggestion,
  useRejectLinkSuggestion,
} from "../../api/queries";
import type {
  AnalyzeLinkSuggestionsResponse,
  LinkSuggestion,
  LinkSuggestionDeclineRecord,
} from "../../api/types";

interface Props {
  readonly projectSlug: string;
}

/// Phase 12a.2 "Suggested links" tab on the Project detail page.
/// Two sub-tabs (Pending / Rejected), per-row Accept/Reject/
/// Reinstate actions, and a manual "Analyze and suggest links"
/// button that runs the LLM-driven proposal pass.
export function SuggestedLinksTab({ projectSlug }: Props) {
  const [subtab, setSubtab] = useState<"pending" | "rejected">("pending");
  const [analyzeStatus, setAnalyzeStatus] =
    useState<AnalyzeLinkSuggestionsResponse | null>(null);

  const pending = usePendingLinkSuggestions(projectSlug);
  const declined = useDeclinedLinkSuggestions(projectSlug);
  const providers = useLlmProviders();
  const analyze = useAnalyzeLinkSuggestions(projectSlug);

  const llmConfigured = (providers.data?.providers.length ?? 0) > 0;

  const onAnalyze = () => {
    setAnalyzeStatus(null);
    analyze.mutate(undefined, {
      onSuccess: (resp) => setAnalyzeStatus(resp),
    });
  };

  return (
    <section
      aria-labelledby="suggested-links-heading"
      data-testid="suggested-links-tab"
      className="space-y-4"
    >
      <header className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <SubTabButton
            active={subtab === "pending"}
            onClick={() => setSubtab("pending")}
            data-testid="suggested-links-pending-tab"
            count={pending.data?.suggestions.length}
          >
            Pending
          </SubTabButton>
          <SubTabButton
            active={subtab === "rejected"}
            onClick={() => setSubtab("rejected")}
            data-testid="suggested-links-rejected-tab"
            count={declined.data?.declined.length}
          >
            Rejected
          </SubTabButton>
        </div>
        <button
          type="button"
          onClick={onAnalyze}
          disabled={!llmConfigured || analyze.isPending}
          title={
            llmConfigured
              ? undefined
              : "Configure an LLM provider in /llm to enable analysis"
          }
          data-testid="analyze-link-suggestions"
          className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
        >
          {analyze.isPending ? "Analyzing…" : "Analyze and suggest links"}
        </button>
      </header>

      <AnalyzeStatusBanner status={analyzeStatus} />
      {analyze.isError && !analyzeStatus ? (
        <div
          role="alert"
          data-testid="analyze-status-error"
          className="rounded border border-rose-300 bg-rose-50 p-3 text-sm text-rose-900 dark:border-rose-700 dark:bg-rose-900/30 dark:text-rose-100"
        >
          Analysis failed: {String(analyze.error)}
        </div>
      ) : null}

      {subtab === "pending" ? (
        <PendingList projectSlug={projectSlug} query={pending} />
      ) : (
        <RejectedList projectSlug={projectSlug} query={declined} />
      )}
    </section>
  );
}

interface SubTabButtonProps {
  readonly active: boolean;
  readonly onClick: () => void;
  readonly children: React.ReactNode;
  readonly count?: number;
  readonly "data-testid"?: string;
}

function SubTabButton({
  active,
  onClick,
  children,
  count,
  "data-testid": testid,
}: SubTabButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={testid}
      aria-pressed={active}
      className={
        active
          ? "rounded border border-slate-900 bg-slate-900 px-3 py-1 text-sm text-white dark:border-slate-100 dark:bg-slate-100 dark:text-slate-900"
          : "rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
      }
    >
      {children}
      {typeof count === "number" ? (
        <span className="ml-2 text-xs opacity-70">({count})</span>
      ) : null}
    </button>
  );
}

interface AnalyzeStatusBannerProps {
  readonly status: AnalyzeLinkSuggestionsResponse | null;
}

function AnalyzeStatusBanner({ status }: AnalyzeStatusBannerProps) {
  if (!status) return null;
  if (status.kind === "ok") {
    return (
      <div
        role="status"
        data-testid="analyze-status-ok"
        className="rounded border border-emerald-300 bg-emerald-50 p-3 text-sm text-emerald-900 dark:border-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-100"
      >
        Generated <strong>{status.suggestions.length}</strong> link suggestion
        {status.suggestions.length === 1 ? "" : "s"} via{" "}
        <span className="font-mono">{status.servedBy}</span>.
      </div>
    );
  }
  if (status.kind === "noProviders") {
    return (
      <div
        role="alert"
        data-testid="analyze-status-noproviders"
        className="rounded border border-amber-300 bg-amber-50 p-3 text-sm text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100"
      >
        No LLM provider is configured. Configure one in the System config (see{" "}
        <span className="font-mono">/llm</span>) before running analysis.
      </div>
    );
  }
  // privacyAckRequired
  return (
    <div
      role="alert"
      data-testid="analyze-status-privacyack"
      className="rounded border border-amber-300 bg-amber-50 p-3 text-sm text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100"
    >
      Acknowledge the privacy notice for provider
      {status.indices.length === 1 ? "" : "s"}{" "}
      <span className="font-mono">{status.indices.join(", ")}</span> before
      running analysis. Visit <span className="font-mono">/llm</span> to
      acknowledge.
    </div>
  );
}

interface PendingListProps {
  readonly projectSlug: string;
  readonly query: ReturnType<typeof usePendingLinkSuggestions>;
}

function PendingList({ projectSlug, query }: PendingListProps) {
  if (query.isLoading) {
    return <p className="text-sm text-slate-500">Loading pending…</p>;
  }
  if (query.isError) {
    return (
      <p className="text-sm text-rose-600" role="alert">
        Could not load pending suggestions: {String(query.error)}
      </p>
    );
  }
  const suggestions = query.data?.suggestions ?? [];
  if (suggestions.length === 0) {
    return (
      <p
        className="text-sm text-slate-600 dark:text-slate-400"
        data-testid="suggested-links-pending-empty"
      >
        No pending suggestions. Click <em>Analyze and suggest links</em> to run
        the LLM-assisted analysis.
      </p>
    );
  }
  return (
    <ul className="space-y-2" data-testid="suggested-links-pending-list">
      {suggestions.map((s) => (
        <SuggestionRow key={s.id} projectSlug={projectSlug} suggestion={s} />
      ))}
    </ul>
  );
}

interface RejectedListProps {
  readonly projectSlug: string;
  readonly query: ReturnType<typeof useDeclinedLinkSuggestions>;
}

function RejectedList({ projectSlug, query }: RejectedListProps) {
  if (query.isLoading) {
    return <p className="text-sm text-slate-500">Loading rejected…</p>;
  }
  if (query.isError) {
    return (
      <p className="text-sm text-rose-600" role="alert">
        Could not load rejected suggestions: {String(query.error)}
      </p>
    );
  }
  const declined = query.data?.declined ?? [];
  if (declined.length === 0) {
    return (
      <p
        className="text-sm text-slate-600 dark:text-slate-400"
        data-testid="suggested-links-rejected-empty"
      >
        No rejected suggestions. Pairs you reject from the Pending tab show up
        here so you can reinstate them later if context changes.
      </p>
    );
  }
  return (
    <ul className="space-y-2" data-testid="suggested-links-rejected-list">
      {declined.map((r) => (
        <DeclinedRow key={r.id} projectSlug={projectSlug} record={r} />
      ))}
    </ul>
  );
}

interface SuggestionRowProps {
  readonly projectSlug: string;
  readonly suggestion: LinkSuggestion;
}

function SuggestionRow({ projectSlug, suggestion }: SuggestionRowProps) {
  // One mutation hook per row so the per-row Accept/Reject
  // pending state doesn't leak to siblings.
  const accept = useAcceptLinkSuggestion(projectSlug);
  const reject = useRejectLinkSuggestion(projectSlug);
  const onAccept = () => accept.mutate(suggestion.id);
  const onReject = () => reject.mutate(suggestion.id);
  const accepting = accept.isPending;
  const rejecting = reject.isPending;
  const confidencePct = Math.round(suggestion.confidence * 100);
  return (
    <li
      data-testid="suggested-link-row"
      className="rounded border border-slate-200 bg-white p-3 dark:border-slate-700 dark:bg-slate-900"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <p className="font-mono text-sm">
            <span className="truncate">{suggestion.from}</span>
            <span className="mx-2 text-slate-400">→</span>
            <span className="truncate">{suggestion.to}</span>
          </p>
          <p className="mt-1 text-sm">
            <span className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-xs dark:bg-slate-800">
              {suggestion.linkType}
            </span>
            <span className="ml-2 text-xs text-slate-500">
              {confidencePct}% confidence
            </span>
          </p>
          {suggestion.rationale ? (
            <p className="mt-2 text-sm text-slate-700 dark:text-slate-300">
              {suggestion.rationale}
            </p>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <button
            type="button"
            onClick={onAccept}
            disabled={accepting || rejecting}
            data-testid="suggested-link-accept"
            className="rounded bg-emerald-600 px-3 py-1 text-sm text-white hover:bg-emerald-700 disabled:opacity-50"
          >
            {accepting ? "Accepting…" : "Accept"}
          </button>
          <button
            type="button"
            onClick={onReject}
            disabled={accepting || rejecting}
            data-testid="suggested-link-reject"
            className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 disabled:opacity-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            {rejecting ? "Rejecting…" : "Reject"}
          </button>
        </div>
      </div>
    </li>
  );
}

interface DeclinedRowProps {
  readonly projectSlug: string;
  readonly record: LinkSuggestionDeclineRecord;
}

function DeclinedRow({ projectSlug, record }: DeclinedRowProps) {
  const reinstate = useReinstateLinkSuggestion(projectSlug);
  const onReinstate = () => reinstate.mutate(record.id);
  const reinstating = reinstate.isPending;
  const confidencePct = Math.round(record.confidence * 100);
  const declinedAt = formatDeclinedAt(record.declinedAt);
  return (
    <li
      data-testid="declined-suggestion-row"
      className="rounded border border-slate-200 bg-slate-50 p-3 dark:border-slate-700 dark:bg-slate-800"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <p className="font-mono text-sm">
            <span className="truncate">{record.from}</span>
            <span className="mx-2 text-slate-400">→</span>
            <span className="truncate">{record.to}</span>
          </p>
          <p className="mt-1 text-sm">
            <span className="rounded bg-slate-200 px-1.5 py-0.5 font-mono text-xs dark:bg-slate-700">
              {record.linkType}
            </span>
            <span className="ml-2 text-xs text-slate-500">
              {confidencePct}% confidence · rejected {declinedAt}
            </span>
          </p>
          {record.rationale ? (
            <p className="mt-2 text-sm text-slate-700 dark:text-slate-300">
              {record.rationale}
            </p>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <button
            type="button"
            onClick={onReinstate}
            disabled={reinstating}
            data-testid="suggested-link-reinstate"
            className="rounded bg-emerald-600 px-3 py-1 text-sm text-white hover:bg-emerald-700 disabled:opacity-50"
          >
            {reinstating ? "Reinstating…" : "Reinstate"}
          </button>
        </div>
      </div>
    </li>
  );
}

function formatDeclinedAt(iso: string): string {
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return iso;
  }
}
