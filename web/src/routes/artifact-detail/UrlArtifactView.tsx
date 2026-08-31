import { useCheckUrl } from "../../api/queries";
import type { ArtifactDetail, UrlCheckStatus } from "../../api/types";

interface Props {
  readonly artifact: ArtifactDetail;
}

/// Colour pallete driven by the stable status strings from
/// UX-urlArtifactChecking / src/urls/check.rs. New outcomes from a
/// future backend roll into the `other` bucket without crashing.
const STATUS_STYLE: Record<
  UrlCheckStatus | "other",
  { label: string; classes: string }
> = {
  ok: {
    label: "OK",
    classes:
      "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200",
  },
  redirect: {
    label: "Redirect",
    classes: "bg-sky-100 text-sky-800 dark:bg-sky-900/40 dark:text-sky-200",
  },
  "not-found": {
    label: "404 not found",
    classes: "bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200",
  },
  forbidden: {
    label: "403 forbidden",
    classes: "bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200",
  },
  unauthorized: {
    label: "401 unauthorized",
    classes:
      "bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200",
  },
  "server-error": {
    label: "5xx server error",
    classes:
      "bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200",
  },
  timeout: {
    label: "Timed out",
    classes:
      "bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200",
  },
  "dns-error": {
    label: "DNS error",
    classes: "bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200",
  },
  "tls-error": {
    label: "TLS error",
    classes: "bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200",
  },
  other: {
    label: "Unreachable",
    classes:
      "bg-slate-200 text-slate-800 dark:bg-slate-800 dark:text-slate-200",
  },
};

function statusPresentation(status: UrlCheckStatus | undefined) {
  if (!status) return null;
  return STATUS_STYLE[status] ?? STATUS_STYLE.other;
}

function formatCheckedAt(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function UrlArtifactView({ artifact }: Props) {
  const mutation = useCheckUrl(artifact.uuid);
  const url = artifact.url ?? "";
  const status = statusPresentation(artifact.checkStatus);

  return (
    <div className="space-y-4">
      <div className="rounded border border-slate-200 bg-slate-50 p-4 dark:border-slate-800 dark:bg-slate-900">
        <p className="text-xs uppercase tracking-wide text-slate-500">
          External URL
        </p>
        <p className="mt-1 break-all">
          {url ? (
            <a
              href={url}
              target="_blank"
              rel="noreferrer noopener"
              className="text-sky-700 underline hover:text-sky-900 dark:text-sky-300"
            >
              {url}
            </a>
          ) : (
            <span className="text-slate-500">No URL set.</span>
          )}
        </p>
        <div className="mt-3 flex flex-wrap items-center gap-3 text-sm">
          {status ? (
            <span
              className={`inline-block rounded px-2 py-0.5 text-xs font-medium ${status.classes}`}
              aria-label={`check status ${artifact.checkStatus}`}
            >
              {status.label}
            </span>
          ) : (
            <span className="text-slate-500">Never checked.</span>
          )}
          {artifact.checkedAt ? (
            <span className="text-xs text-slate-500">
              Last checked {formatCheckedAt(artifact.checkedAt)}
            </span>
          ) : null}
          <button
            type="button"
            onClick={() => mutation.mutate()}
            disabled={mutation.isPending || !url}
            className="ml-auto rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-100 disabled:opacity-50 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            {mutation.isPending ? "Checking…" : "Check URL now"}
          </button>
        </div>
        {mutation.error ? (
          <p className="mt-2 text-sm text-rose-600" role="alert">
            {String(mutation.error)}
          </p>
        ) : null}
      </div>
      <p className="text-xs text-slate-500">
        URL checks run only on demand. A failing check records the status but
        does not deactivate the artifact.
      </p>
    </div>
  );
}
