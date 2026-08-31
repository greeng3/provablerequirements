import { useId, useMemo, useState } from "react";

import { useLinkTypes, useMatrix } from "../../../api/queries";
import type {
  MatrixEdgeDto,
  MatrixNodeDto,
  MatrixReviewStateTag,
  ReportScopeParam,
} from "../../../api/types";
import { MATRIX_AXIS_CAP } from "../../../api/types";

import { GraphToast } from "../graph/GraphToast";
import { MatrixAxisFilters } from "./MatrixAxisFilters";
import { MatrixCellDialog } from "./MatrixCellDialog";
import { MatrixGrid } from "./MatrixGrid";

/// Default link type for new matrix sessions. Matches the
/// Phase 6a coverage-matrix report's default covering set
/// ({satisfies, verifies}) so operators landing on the matrix
/// see the same working set they tune in the report.
const DEFAULT_LINK_TYPE = "satisfies";

interface PendingCell {
  readonly row: MatrixNodeDto;
  readonly column: MatrixNodeDto;
  readonly filled: boolean;
}

interface ToastState {
  readonly message: string;
  readonly tone: "info" | "error";
}

/// Matrix view. 7b.2 shipped the read-only render; 7b.3 adds
/// cell-click authoring via a confirmation modal that rewrites
/// the row artifact's links through `PUT /api/artifacts/:uuid`.
export function MatrixPage() {
  const [rowScope, setRowScope] = useState<ReportScopeParam>("system");
  const [columnScope, setColumnScope] = useState<ReportScopeParam>("system");
  const [linkType, setLinkType] = useState<string>(DEFAULT_LINK_TYPE);
  const [includeInactive, setIncludeInactive] = useState(false);
  const [rowTags, setRowTags] = useState<string[]>([]);
  const [columnTags, setColumnTags] = useState<string[]>([]);
  const [rowReviewStates, setRowReviewStates] = useState<
    MatrixReviewStateTag[]
  >([]);
  const [columnReviewStates, setColumnReviewStates] = useState<
    MatrixReviewStateTag[]
  >([]);

  const linkTypesQuery = useLinkTypes();

  const params = useMemo(
    () => ({
      rowScope,
      columnScope,
      linkType,
      includeInactive,
      rowTags: rowTags.length > 0 ? rowTags : undefined,
      columnTags: columnTags.length > 0 ? columnTags : undefined,
      rowReviewStates: rowReviewStates.length > 0 ? rowReviewStates : undefined,
      columnReviewStates:
        columnReviewStates.length > 0 ? columnReviewStates : undefined,
    }),
    [
      rowScope,
      columnScope,
      linkType,
      includeInactive,
      rowTags,
      columnTags,
      rowReviewStates,
      columnReviewStates,
    ],
  );

  const matrixQuery = useMatrix(params);
  const linkSelectId = useId();

  const matrixData = matrixQuery.data;

  const [pendingCell, setPendingCell] = useState<PendingCell | undefined>(
    undefined,
  );
  const [toast, setToast] = useState<ToastState | undefined>(undefined);

  const edgeIndex = useMemo(() => {
    const s = new Set<string>();
    for (const e of matrixData?.edges ?? []) {
      s.add(`${e.rowUuid}\u0000${e.columnUuid}`);
    }
    return s;
  }, [matrixData?.edges]);

  const onCellClick = (row: MatrixNodeDto, column: MatrixNodeDto) => {
    const filled = edgeIndex.has(`${row.uuid}\u0000${column.uuid}`);
    setPendingCell({ row, column, filled });
  };

  const onToggled = (action: "created" | "removed") => {
    setToast({
      message:
        action === "created"
          ? `Link created (${linkType}).`
          : `Link removed (${linkType}).`,
      tone: "info",
    });
  };

  return (
    <section className="space-y-4">
      <header className="space-y-1 border-b border-slate-200 pb-3 dark:border-slate-800">
        <h1 className="text-2xl font-semibold tracking-tight">Matrix</h1>
        <p className="text-sm text-slate-600 dark:text-slate-400">
          Coverage and gap-analysis view: one axis per side, cells surface the
          chosen link type. Click a cell to create or remove the link. Filter
          each axis to narrow below {MATRIX_AXIS_CAP} items before the grid
          renders.
        </p>
      </header>

      <div className="flex flex-wrap items-center gap-3 border-b border-slate-200 pb-3 dark:border-slate-800">
        <label className="flex items-center gap-2 text-sm">
          <span
            className="text-xs uppercase tracking-wide text-slate-500"
            id={`${linkSelectId}-label`}
          >
            Link type
          </span>
          <select
            id={linkSelectId}
            aria-labelledby={`${linkSelectId}-label`}
            value={linkType}
            onChange={(e) => setLinkType(e.target.value)}
            className="rounded border border-slate-300 bg-white px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
          >
            {linkTypesQuery.isLoading ? (
              <option value={linkType}>{linkType}</option>
            ) : (
              (linkTypesQuery.data ?? []).map((t) => (
                <option key={t.name} value={t.name}>
                  {t.name}
                  {t.acyclic ? " · acyclic" : ""}
                </option>
              ))
            )}
          </select>
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={includeInactive}
            onChange={(e) => setIncludeInactive(e.target.checked)}
          />
          <span>Include inactive</span>
        </label>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <MatrixAxisFilters
          title="Rows (source)"
          scope={rowScope}
          onScopeChange={setRowScope}
          tags={rowTags}
          onTagsChange={setRowTags}
          reviewStates={rowReviewStates}
          onReviewStatesChange={setRowReviewStates}
          nodes={matrixData?.rows ?? []}
        />
        <MatrixAxisFilters
          title="Columns (target)"
          scope={columnScope}
          onScopeChange={setColumnScope}
          tags={columnTags}
          onTagsChange={setColumnTags}
          reviewStates={columnReviewStates}
          onReviewStatesChange={setColumnReviewStates}
          nodes={matrixData?.columns ?? []}
        />
      </div>

      {matrixQuery.isLoading ? (
        <p className="text-sm text-slate-500">Loading matrix…</p>
      ) : matrixQuery.isError || !matrixData ? (
        <p className="text-sm text-rose-600" role="alert">
          Failed to load matrix: {String(matrixQuery.error ?? "unknown")}
        </p>
      ) : (
        <MatrixBody
          rowsTruncated={matrixData.rowsTruncated}
          columnsTruncated={matrixData.columnsTruncated}
          totalRows={matrixData.totalRows}
          totalColumns={matrixData.totalColumns}
          rows={matrixData.rows}
          columns={matrixData.columns}
          edges={matrixData.edges}
          linkTypeName={matrixData.linkType.name}
          onCellClick={onCellClick}
        />
      )}

      {pendingCell ? (
        <MatrixCellDialog
          row={pendingCell.row}
          column={pendingCell.column}
          linkType={linkType}
          initialFilled={pendingCell.filled}
          onClose={() => setPendingCell(undefined)}
          onToggled={onToggled}
        />
      ) : null}

      {toast ? (
        <GraphToast
          message={toast.message}
          tone={toast.tone}
          onDismiss={() => setToast(undefined)}
        />
      ) : null}
    </section>
  );
}

interface BodyProps {
  readonly rowsTruncated: boolean;
  readonly columnsTruncated: boolean;
  readonly totalRows: number;
  readonly totalColumns: number;
  readonly rows: MatrixNodeDto[];
  readonly columns: MatrixNodeDto[];
  readonly edges: MatrixEdgeDto[];
  readonly linkTypeName: string;
  readonly onCellClick: (row: MatrixNodeDto, column: MatrixNodeDto) => void;
}

function MatrixBody({
  rowsTruncated,
  columnsTruncated,
  totalRows,
  totalColumns,
  rows,
  columns,
  edges,
  linkTypeName,
  onCellClick,
}: BodyProps) {
  if (rowsTruncated || columnsTruncated) {
    return (
      <p
        role="alert"
        data-testid="matrix-truncation-banner"
        className="rounded border border-amber-300 bg-amber-50 p-3 text-sm text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100"
      >
        {buildTruncationMessage(
          rowsTruncated,
          columnsTruncated,
          totalRows,
          totalColumns,
        )}
      </p>
    );
  }
  if (rows.length === 0 || columns.length === 0) {
    return (
      <p className="text-sm text-slate-500">
        No artifacts on {rows.length === 0 ? "the row axis" : "the column axis"}
        . Try broadening the scope or clearing the filters.
      </p>
    );
  }
  return (
    <MatrixGrid
      rows={rows}
      columns={columns}
      edges={edges}
      linkTypeName={linkTypeName}
      onCellClick={onCellClick}
    />
  );
}

function buildTruncationMessage(
  rowsTruncated: boolean,
  columnsTruncated: boolean,
  totalRows: number,
  totalColumns: number,
): string {
  const parts: string[] = [];
  if (rowsTruncated) {
    parts.push(`row axis has ${totalRows} items`);
  }
  if (columnsTruncated) {
    parts.push(`column axis has ${totalColumns} items`);
  }
  return `${parts.join(" and ")} — apply filters to narrow below ${MATRIX_AXIS_CAP}.`;
}
