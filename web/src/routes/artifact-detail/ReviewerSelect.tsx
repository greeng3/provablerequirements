import { useEffect, useMemo, useState, type ChangeEvent } from "react";

import { useReviewers } from "../../api/queries";

/// Dropdown of reviewer identities for the review pane, per
/// `REVIEW-reviewerIdentity`. Groups the options into "Default
/// (git)", "Used this session", and "Persisted" so the operator
/// can pick the most relevant one at a glance. Falls back to a
/// free-text input when the desired identity isn't in any of the
/// three lists.
///
/// The parent owns the selected value and passes the current
/// reviewer down via `value` / `onChange` — same shape other form
/// controls in this pane use.
export function ReviewerSelect({
  projectSlug,
  value,
  onChange,
}: {
  projectSlug?: string;
  value: string;
  onChange: (next: string) => void;
}) {
  const query = useReviewers(projectSlug);
  const [mode, setMode] = useState<"menu" | "custom">("menu");

  const options = useMemo(() => {
    const data = query.data;
    if (!data) return { groups: [], flat: new Set<string>() };
    const groups: Array<{ label: string; values: string[] }> = [];
    const flat = new Set<string>();
    if (data.gitDefault) {
      groups.push({ label: "Default (git)", values: [data.gitDefault] });
      flat.add(data.gitDefault);
    }
    if (data.session.length > 0) {
      const sessionValues = data.session.filter((v) => !flat.has(v));
      if (sessionValues.length > 0) {
        groups.push({ label: "Used this session", values: sessionValues });
        for (const v of sessionValues) flat.add(v);
      }
    }
    if (data.persisted.length > 0) {
      const persistedValues = data.persisted.filter((v) => !flat.has(v));
      if (persistedValues.length > 0) {
        groups.push({ label: "Persisted", values: persistedValues });
        for (const v of persistedValues) flat.add(v);
      }
    }
    return { groups, flat };
  }, [query.data]);

  // Pre-populate with the git default on first successful load,
  // unless the parent already set a value (which would be the case
  // after a previous submission).
  useEffect(() => {
    if (!query.data) return;
    if (value !== "" || !query.data.gitDefault) return;
    onChange(query.data.gitDefault);
  }, [query.data, value, onChange]);

  // When there are no preset options at all (fresh container, no
  // git default, no prior reviewers), the dropdown collapses to a
  // lone "Type a new reviewer…" entry — which is awkward because
  // the operator has to actively click it to swap into text-input
  // mode. Skip the trick: render the text input directly.
  const hasPresets = options.groups.length > 0;
  if (mode === "custom" || !hasPresets) {
    return (
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="New reviewer identity"
          className="flex-1 rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          aria-label="Reviewer identity"
        />
        {hasPresets ? (
          <button
            type="button"
            onClick={() => setMode("menu")}
            className="text-xs text-slate-500 hover:underline"
          >
            Pick from list
          </button>
        ) : null}
      </div>
    );
  }

  const handle = (e: ChangeEvent<HTMLSelectElement>) => {
    const next = e.target.value;
    if (next === "__custom__") {
      setMode("custom");
      return;
    }
    onChange(next);
  };

  return (
    <div className="flex items-center gap-2">
      <select
        value={value}
        onChange={handle}
        aria-label="Reviewer identity"
        className="flex-1 rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
      >
        {value !== "" && !options.flat.has(value) && (
          <option value={value}>{value}</option>
        )}
        {options.groups.map((group) => (
          <optgroup key={group.label} label={group.label}>
            {group.values.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </optgroup>
        ))}
        <option value="__custom__">Type a new reviewer…</option>
      </select>
    </div>
  );
}
