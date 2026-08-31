import type { BulkCheckUrlsResponse } from "../../api/types";

interface Props {
  readonly urlArtifactCount: number;
  readonly pending: boolean;
  readonly result?: BulkCheckUrlsResponse;
  readonly error: unknown;
  readonly onCheck: () => void;
}

/// Renders the collection-level "Check URLs" button plus its
/// in-place status. Kept split from `CollectionPage` because the
/// small live-progress / summary readout is what the Phase 5c UX
/// actually wants — a button alone would hide the outcome behind
/// a reload.
export function BulkUrlCheckButton({
  urlArtifactCount,
  pending,
  result,
  error,
  onCheck,
}: Props) {
  const label = pending
    ? "Checking…"
    : `Check ${urlArtifactCount} URL${urlArtifactCount === 1 ? "" : "s"}`;
  return (
    <div className="flex flex-col items-end gap-1">
      <button
        type="button"
        onClick={onCheck}
        disabled={pending}
        className="rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-100 disabled:opacity-50 dark:border-slate-600 dark:hover:bg-slate-800"
      >
        {label}
      </button>
      {result ? <BulkCheckSummary result={result} /> : null}
      {error ? (
        <p className="text-xs text-rose-600" role="alert">
          {String(error)}
        </p>
      ) : null}
    </div>
  );
}

function BulkCheckSummary({ result }: { result: BulkCheckUrlsResponse }) {
  const total = result.checked.length;
  const ok = result.checked.filter((c) => c.checkStatus === "ok").length;
  const failed = total - ok;
  return (
    <p className="text-xs text-slate-500" role="status">
      {ok}/{total} OK · {failed} failure{failed === 1 ? "" : "s"}
    </p>
  );
}
