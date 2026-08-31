import { useMemo } from "react";

import { useLastApprovalSnapshot } from "../../api/queries";
import type { ArtifactDiffResponse, DiffLine } from "../../api/types";
import { DiffView } from "./DiffView";

/// Body and metadata diff between the current artifact and the
/// body/metadata captured at the last approval snapshot. Part of
/// `UX-reviewPane`'s "Since last approval" section.
///
/// Phase 5d: the rendering is delegated to [`DiffView`] so the
/// review pane and the standalone /artifacts/:uuid/diff route
/// share the same visual vocabulary. The diff itself still runs
/// client-side over the approval-snapshot payload because the
/// snapshot is the operator's approved reference body — no git
/// round-trip required for the "since last approval" surface
/// (the banner wording from the locked decision applies when the
/// snapshot is absent altogether).
export function SinceLastApprovalDiff({
  uuid,
  enabled,
  currentBody,
  currentMetadata,
}: {
  uuid: string;
  enabled: boolean;
  currentBody: string;
  currentMetadata: CurrentMetadataSummary;
}) {
  const snapshot = useLastApprovalSnapshot(uuid, enabled);

  if (!enabled) return null;
  if (snapshot.isLoading) {
    return <p className="text-sm text-slate-500">Loading snapshot…</p>;
  }
  if (snapshot.isError || !snapshot.data) {
    return (
      <p className="text-sm text-slate-500">
        Snapshot not available for this approval.
      </p>
    );
  }

  const before = snapshot.data.body;
  const after = currentBody;
  const metadataRows = metadataDiffRows(
    parseMetadata(snapshot.data.metadata),
    currentMetadata,
  );
  return (
    <div className="space-y-3">
      <ApprovalBodyDiff
        before={before}
        after={after}
        beforeLabel={`approval · ${snapshot.data.approvedAt.slice(0, 10)}`}
      />
      <MetadataDiff rows={metadataRows} />
    </div>
  );
}

function parseMetadata(raw: Record<string, unknown>): CurrentMetadataSummary {
  return {
    title: typeof raw["title"] === "string" ? (raw["title"] as string) : "",
    description:
      typeof raw["description"] === "string"
        ? (raw["description"] as string)
        : null,
    tags: Array.isArray(raw["tags"])
      ? (raw["tags"] as unknown[]).filter(
          (t): t is string => typeof t === "string",
        )
      : [],
    outlineLevel:
      typeof raw["outlineLevel"] === "string"
        ? (raw["outlineLevel"] as string)
        : undefined,
    active: raw["active"] !== false,
    derived: raw["derived"] === true,
  };
}

type MetadataRow = { label: string; before: string; after: string };

function metadataDiffRows(
  before: CurrentMetadataSummary,
  after: CurrentMetadataSummary,
): MetadataRow[] {
  const rows: MetadataRow[] = [];
  const push = (label: string, b: string, a: string) => {
    if (b !== a) rows.push({ label, before: b, after: a });
  };
  push("title", before.title, after.title);
  push("description", before.description ?? "", after.description ?? "");
  push("tags", before.tags.join(", "), after.tags.join(", "));
  push("outlineLevel", before.outlineLevel ?? "", after.outlineLevel ?? "");
  push("active", String(before.active), String(after.active));
  push("derived", String(before.derived), String(after.derived));
  return rows;
}

function MetadataDiff({ rows }: { rows: MetadataRow[] }) {
  if (rows.length === 0) {
    return (
      <p className="text-sm text-slate-500">
        Metadata unchanged since approval.
      </p>
    );
  }
  return (
    <table
      className="w-full border-collapse text-xs"
      aria-label="Metadata diff"
    >
      <thead>
        <tr className="text-left text-slate-500">
          <th className="py-1">Field</th>
          <th className="py-1">Before</th>
          <th className="py-1">After</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr
            key={r.label}
            className="border-t border-slate-200 dark:border-slate-700"
          >
            <td className="py-1 pr-2 font-mono text-slate-600">{r.label}</td>
            <td className="py-1 pr-2 text-slate-500 line-through">
              {r.before || "\u00A0"}
            </td>
            <td className="py-1 text-slate-800 dark:text-slate-200">
              {r.after || "\u00A0"}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function ApprovalBodyDiff({
  before,
  after,
  beforeLabel,
}: {
  before: string;
  after: string;
  beforeLabel: string;
}) {
  const response: ArtifactDiffResponse = useMemo(() => {
    return {
      shape: "content",
      fromLabel: beforeLabel,
      toLabel: "current draft",
      diff: { shape: "content", lines: diffLines(before, after) },
    };
  }, [before, after, beforeLabel]);
  return <DiffView response={response} />;
}

export interface CurrentMetadataSummary {
  title: string;
  description?: string | null;
  tags: string[];
  outlineLevel?: string;
  active: boolean;
  derived: boolean;
}

// --- LCS-based line diff, kept client-side so the approval
//     snapshot can render without a git round-trip. The inlined
//     implementation is a deliberate duplicate of the backend's
//     `similar`-based diff — the two converge on the same
//     wire-level output for same/added/removed lines.

function diffLines(before: string, after: string): DiffLine[] {
  const aLines = before.split("\n");
  const bLines = after.split("\n");
  const n = aLines.length;
  const m = bLines.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () =>
    new Array(m + 1).fill(0),
  );
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] =
        aLines[i] === bLines[j]
          ? dp[i + 1][j + 1] + 1
          : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (aLines[i] === bLines[j]) {
      out.push({ kind: "same", text: aLines[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ kind: "removed", text: aLines[i] });
      i++;
    } else {
      out.push({ kind: "added", text: bLines[j] });
      j++;
    }
  }
  while (i < n) {
    out.push({ kind: "removed", text: aLines[i++] });
  }
  while (j < m) {
    out.push({ kind: "added", text: bLines[j++] });
  }
  return out;
}
