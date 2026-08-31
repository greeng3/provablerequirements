import { useEffect, useState } from "react";

import { useSystemState } from "../../api/queries";

const STORAGE_PREFIX = "reqforge.systemConfigBanner.dismissed.";

/// Phase 11c / UX-systemConfigBanner: nudges the operator to
/// author a System config when two or more projects are mounted
/// but none is loaded. ReqForge never writes a System config
/// unbidden — the banner is guidance, not a wizard.
///
/// Dismissal is per-session (via `sessionStorage`) keyed by the
/// current `projectCount`. If another project is mounted later
/// in the session, the banner re-shows — this is deliberate:
/// the "we should probably have a System" calculus changes when
/// the mount set changes.
export function SystemConfigBanner() {
  const query = useSystemState();
  const [dismissed, setDismissed] = useState(false);

  const data = query.data;
  const shouldRender = !!data && !data.loaded && data.projectCount >= 2;
  const storageKey = data ? `${STORAGE_PREFIX}${data.projectCount}` : "";

  useEffect(() => {
    if (!storageKey) {
      setDismissed(false);
      return;
    }
    try {
      setDismissed(sessionStorage.getItem(storageKey) === "1");
    } catch {
      // Ignore storage errors — sessionStorage can be absent or
      // blocked. Banner just stays visible; that's fine.
      setDismissed(false);
    }
  }, [storageKey]);

  if (!shouldRender || dismissed) return null;

  const dismiss = () => {
    try {
      sessionStorage.setItem(storageKey, "1");
    } catch {
      // Ignore storage errors — state still updates below.
    }
    setDismissed(true);
  };

  return (
    <div
      role="status"
      data-testid="system-config-banner"
      className="mb-6 flex items-start justify-between gap-3 rounded border border-sky-300 bg-sky-50 p-3 text-sm text-sky-900 dark:border-sky-700 dark:bg-sky-900/30 dark:text-sky-100"
    >
      <div>
        <p className="font-semibold">
          {data!.projectCount} projects are mounted but no System config is
          loaded.
        </p>
        <p className="mt-1">
          A System config groups related projects under a shared name, enables
          cross-project reports, and declares a common link-type catalog. See
          the{" "}
          <code className="rounded bg-white/60 px-1 py-0.5 font-mono text-xs dark:bg-slate-800/60">
            REQFORGE_SYSTEM_CONFIG
          </code>{" "}
          environment variable in the deployment docs. ReqForge never writes one
          for you.
        </p>
      </div>
      <button
        type="button"
        onClick={dismiss}
        data-testid="system-config-banner-dismiss"
        className="shrink-0 rounded border border-sky-300 bg-white px-2 py-1 text-xs font-medium hover:bg-sky-100 dark:border-sky-700 dark:bg-slate-900 dark:hover:bg-slate-800"
      >
        Dismiss
      </button>
    </div>
  );
}
