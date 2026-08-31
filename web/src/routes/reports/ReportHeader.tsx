import { useEffect } from "react";

import {
  useClearReportConfig,
  useReportConfig,
  useSaveReportConfig,
} from "../../api/queries";
import type { ReportKind, ReportScopeParam } from "../../api/types";

import { ExportMenu } from "./ExportMenu";
import { ScopeSelector } from "./ScopeSelector";

interface Props {
  readonly kind: ReportKind;
  readonly title: string;
  readonly description?: string;
  readonly scope: ReportScopeParam;
  readonly includeInactive: boolean;
  readonly onScopeChange: (scope: ReportScopeParam) => void;
  readonly onIncludeInactiveChange: (value: boolean) => void;
  readonly onResetToDefaults: () => void;
  readonly showSavedConfigHint?: boolean;
  /// Per-kind extra params threaded into the Phase 6b export
  /// links so downloads match the currently-rendered report:
  /// coverage-matrix's covering link types, impact-analysis's
  /// seed + direction, etc. Absent from list reports whose only
  /// inputs are scope + includeInactive.
  readonly exportExtras?: Record<string, string | undefined>;
}

/// Shared header for every report page: title + scope selector +
/// inactive-toggle + reset-config button, hydrated from the
/// per-kind saved config on mount.
export function ReportHeader({
  kind,
  title,
  description,
  scope,
  includeInactive,
  onScopeChange,
  onIncludeInactiveChange,
  onResetToDefaults,
  exportExtras,
}: Props) {
  const saved = useReportConfig(kind);
  const save = useSaveReportConfig(kind);
  const clearConfig = useClearReportConfig(kind);

  // First time we see a non-empty saved config, apply it to the
  // current state. Subsequent state changes persist via the
  // onScopeChange / onIncludeInactiveChange callers.
  useEffect(() => {
    if (!saved.data) return;
    if (saved.data.scope !== undefined && saved.data.scope !== scope) {
      onScopeChange(saved.data.scope);
    }
    if (
      saved.data.includeInactive !== undefined &&
      saved.data.includeInactive !== includeInactive
    ) {
      onIncludeInactiveChange(saved.data.includeInactive);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [saved.data?.scope, saved.data?.includeInactive]);

  // Persist whenever scope / inactive flag change after the
  // initial hydration.
  useEffect(() => {
    if (saved.isLoading) return;
    save.mutate({
      ...(saved.data ?? {}),
      scope,
      includeInactive,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scope, includeInactive]);

  const handleReset = () => {
    clearConfig.mutate(undefined, {
      onSuccess: () => {
        onResetToDefaults();
      },
    });
  };

  return (
    <header className="space-y-3 border-b border-slate-200 pb-4 dark:border-slate-800">
      <div className="flex items-baseline justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
          {description ? (
            <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
              {description}
            </p>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-3">
          <ExportMenu
            kind={kind}
            scope={scope}
            includeInactive={includeInactive}
            extra={exportExtras}
          />
          <button
            type="button"
            onClick={handleReset}
            className="rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Reset to defaults
          </button>
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-6">
        <ScopeSelector value={scope} onChange={onScopeChange} />
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={includeInactive}
            onChange={(e) => onIncludeInactiveChange(e.target.checked)}
          />
          <span>Include inactive artifacts</span>
        </label>
      </div>
    </header>
  );
}
