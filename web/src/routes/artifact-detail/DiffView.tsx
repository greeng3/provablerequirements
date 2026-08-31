import type {
  ArtifactDiffResponse,
  BlobDiffSide,
  DiffLine,
  ShapeDiff,
} from "../../api/types";

interface Props {
  readonly response: ArtifactDiffResponse;
}

/// Renders the shape-tagged `/diff` response. Called from both the
/// standalone /artifacts/:uuid/diff route and the review pane's
/// "Since last approval" block, so the visual vocabulary is
/// unified across the two surfaces.
export function DiffView({ response }: Props) {
  return (
    <div className="space-y-3">
      <DiffHeader response={response} />
      {response.fallbackReason ? (
        <FallbackBanner reason={response.fallbackReason} />
      ) : null}
      <DiffBody diff={response.diff} />
    </div>
  );
}

function DiffHeader({ response }: { response: ArtifactDiffResponse }) {
  return (
    <p className="text-xs text-slate-500">
      <span className="font-mono">{response.fromLabel}</span>
      <span className="mx-1">→</span>
      <span className="font-mono">{response.toLabel}</span>
    </p>
  );
}

function FallbackBanner({ reason }: { reason: string }) {
  return (
    <div
      role="alert"
      className="rounded border border-amber-300 bg-amber-50 p-2 text-xs text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100"
    >
      Historical git context unavailable; diff computed against last approval
      snapshot instead. Reason: {reason}
    </div>
  );
}

function DiffBody({ diff }: { diff: ShapeDiff }) {
  if (diff.shape === "content") return <ContentDiffPane lines={diff.lines} />;
  if (diff.shape === "blob")
    return <BlobDiffPane before={diff.before} after={diff.after} />;
  return (
    <UrlDiffPane before={diff.before} after={diff.after} note={diff.note} />
  );
}

function ContentDiffPane({ lines }: { lines: DiffLine[] }) {
  const changed = lines.some((l) => l.kind !== "same");
  if (!changed) {
    return <p className="text-sm text-slate-500">No line-level changes.</p>;
  }
  return (
    <div className="overflow-auto rounded border border-slate-200 bg-white text-xs dark:border-slate-700 dark:bg-slate-900">
      <pre className="whitespace-pre-wrap break-words p-2">
        {lines.map((l, idx) => (
          <div
            key={idx}
            className={
              l.kind === "added"
                ? "bg-emerald-100 dark:bg-emerald-900/40"
                : l.kind === "removed"
                  ? "bg-rose-100 text-slate-500 line-through dark:bg-rose-900/40"
                  : ""
            }
          >
            <span className="mr-2 select-none text-slate-400">
              {l.kind === "added" ? "+" : l.kind === "removed" ? "-" : " "}
            </span>
            {l.text || "\u00A0"}
          </div>
        ))}
      </pre>
    </div>
  );
}

function BlobDiffPane({
  before,
  after,
}: {
  before?: BlobDiffSide;
  after?: BlobDiffSide;
}) {
  return (
    <div className="grid gap-3 lg:grid-cols-2">
      <BlobSideCard label="Before" side={before} />
      <BlobSideCard label="After" side={after} />
    </div>
  );
}

function BlobSideCard({ label, side }: { label: string; side?: BlobDiffSide }) {
  return (
    <div className="rounded border border-slate-200 p-3 text-sm dark:border-slate-800">
      <p className="mb-2 text-xs uppercase tracking-wide text-slate-500">
        {label}
      </p>
      {side ? (
        <BlobSideBody side={side} />
      ) : (
        <p className="text-sm text-slate-500">(not present at this commit)</p>
      )}
    </div>
  );
}

function BlobSideBody({ side }: { side: BlobDiffSide }) {
  if (side.mediaType.startsWith("image/")) {
    return (
      <div className="space-y-2">
        <img
          src={side.downloadUrl}
          alt=""
          className="max-h-80 w-auto rounded border border-slate-200 dark:border-slate-800"
        />
        <BlobFacts side={side} />
      </div>
    );
  }
  return (
    <div className="space-y-2">
      <p className="text-slate-700 dark:text-slate-300">
        <span className="font-mono">{side.mediaType}</span>
      </p>
      <BlobFacts side={side} />
      <a
        href={side.downloadUrl}
        className="inline-block rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
        download
      >
        Download
      </a>
    </div>
  );
}

function BlobFacts({ side }: { side: BlobDiffSide }) {
  return (
    <p className="text-xs text-slate-500">
      {formatBytes(side.byteSize)} · sha256{" "}
      <span className="font-mono">{side.contentHash.slice(0, 12)}…</span>
    </p>
  );
}

function UrlDiffPane({
  before,
  after,
  note,
}: {
  before?: string;
  after?: string;
  note: string;
}) {
  return (
    <div className="space-y-2">
      <div className="grid gap-3 lg:grid-cols-2">
        <UrlSideCard label="Before" url={before} />
        <UrlSideCard label="After" url={after} />
      </div>
      <p className="text-xs text-slate-500">{note}</p>
    </div>
  );
}

function UrlSideCard({ label, url }: { label: string; url?: string }) {
  return (
    <div className="rounded border border-slate-200 p-3 text-sm dark:border-slate-800">
      <p className="mb-1 text-xs uppercase tracking-wide text-slate-500">
        {label}
      </p>
      {url ? (
        <a
          href={url}
          target="_blank"
          rel="noreferrer noopener"
          className="break-all text-sky-700 underline dark:text-sky-300"
        >
          {url}
        </a>
      ) : (
        <p className="text-slate-500">(not present at this commit)</p>
      )}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unitIdx = 0;
  while (value >= 1024 && unitIdx < units.length - 1) {
    value /= 1024;
    unitIdx += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unitIdx]}`;
}
