export function EmptyArtifactsState() {
  return (
    <div className="rounded-lg border border-dashed border-slate-300 bg-white p-6 text-sm text-slate-700 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300">
      <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">
        No artifacts yet
      </h2>
      <p className="mt-2">
        This collection contains no artifacts. Add one by dropping a Markdown
        file with JSON frontmatter into the collection directory, or wait for
        the Phase 2 UI to land.
      </p>
    </div>
  );
}
