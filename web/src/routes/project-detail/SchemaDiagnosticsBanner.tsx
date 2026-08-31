import type { SchemaDiagnostic } from "../../api/types";

interface Props {
  readonly diagnostics: SchemaDiagnostic[];
}

/// Phase 11a banner — surfaces per-file "this is newer than I
/// understand" diagnostics at the top of the Project detail
/// page. Hidden when the list is empty, so clean projects
/// don't render anything.
export function SchemaDiagnosticsBanner({ diagnostics }: Props) {
  if (diagnostics.length === 0) return null;
  return (
    <div
      role="alert"
      data-testid="schema-diagnostics-banner"
      className="rounded border border-rose-300 bg-rose-50 p-3 text-sm text-rose-900 dark:border-rose-700 dark:bg-rose-900/30 dark:text-rose-100"
    >
      <p className="font-semibold">
        {diagnostics.length === 1
          ? "One file was written by a newer ReqForge"
          : `${diagnostics.length} files were written by a newer ReqForge`}
      </p>
      <p className="mt-1">
        These files won't load until you upgrade ReqForge. Other files in the
        project continue to work.
      </p>
      <ul className="mt-2 space-y-1 font-mono text-xs">
        {diagnostics.map((d, idx) => (
          <li key={`${d.path}-${idx}`}>
            {d.path} · <span className="italic">{d.fileType}</span> · v
            {d.foundVersion} (this build supports up to v{d.currentVersion})
          </li>
        ))}
      </ul>
    </div>
  );
}
