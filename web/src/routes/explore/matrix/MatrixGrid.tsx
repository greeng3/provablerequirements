import { useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import type { MatrixEdgeDto, MatrixNodeDto } from "../../../api/types";

interface Props {
  readonly rows: MatrixNodeDto[];
  readonly columns: MatrixNodeDto[];
  readonly edges: MatrixEdgeDto[];
  readonly linkTypeName: string;
  /// Invoked when a non-self-link cell is clicked. 7b.2 only
  /// renders the grid; 7b.3 wires this through to the
  /// MatrixCellDialog.
  readonly onCellClick?: (row: MatrixNodeDto, column: MatrixNodeDto) => void;
}

const ROW_HEIGHT = 32;
const COLUMN_WIDTH = 160;
const HEADER_COLUMN_WIDTH = 260;
const HEADER_ROW_HEIGHT = 90;

/// Key a cell by composite row + column UUID so `.has()` lookups
/// stay O(1) per render. Sparse edges (< a few thousand) keep
/// the Set allocation negligible.
function edgeKey(rowUuid: string, columnUuid: string): string {
  return `${rowUuid}\u0000${columnUuid}`;
}

/// TanStack-Virtual-backed grid. Both axes virtualize so a
/// maxed-out 500×500 matrix (250k cells) renders responsively
/// by only mounting the DOM for what's on-screen.
export function MatrixGrid({
  rows,
  columns,
  edges,
  linkTypeName,
  onCellClick,
}: Props) {
  const scrollRef = useRef<HTMLDivElement | null>(null);

  const edgeSet = useMemo(() => {
    const s = new Set<string>();
    for (const e of edges) s.add(edgeKey(e.rowUuid, e.columnUuid));
    return s;
  }, [edges]);

  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 6,
  });

  const columnVirtualizer = useVirtualizer({
    count: columns.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => COLUMN_WIDTH,
    overscan: 4,
    horizontal: true,
  });

  const totalWidth = HEADER_COLUMN_WIDTH + columnVirtualizer.getTotalSize();
  const totalHeight = HEADER_ROW_HEIGHT + rowVirtualizer.getTotalSize();

  return (
    <div
      ref={scrollRef}
      data-testid="matrix-grid"
      className="relative h-[calc(100vh-22rem)] min-h-[28rem] overflow-auto rounded border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900"
    >
      <div
        style={{
          position: "relative",
          width: `${totalWidth}px`,
          height: `${totalHeight}px`,
        }}
      >
        {/* Top-left corner: frozen label + type badge. */}
        <div
          className="sticky top-0 left-0 z-30 flex items-end border-r border-b border-slate-300 bg-slate-50 px-2 py-1 text-xs font-semibold dark:border-slate-700 dark:bg-slate-800"
          style={{
            width: `${HEADER_COLUMN_WIDTH}px`,
            height: `${HEADER_ROW_HEIGHT}px`,
          }}
        >
          <span>
            Rows →
            <span className="mx-1 font-mono text-[11px] text-sky-700 dark:text-sky-300">
              {linkTypeName}
            </span>
            → Columns
          </span>
        </div>

        {/* Column headers — horizontally virtualized, vertically
            pinned to the top. */}
        <div
          className="sticky top-0 z-20"
          style={{
            left: `${HEADER_COLUMN_WIDTH}px`,
            height: `${HEADER_ROW_HEIGHT}px`,
          }}
        >
          {columnVirtualizer.getVirtualItems().map((col) => {
            const node = columns[col.index];
            return (
              <div
                key={col.key}
                data-testid={`matrix-col-header-${node.artifactName}`}
                className="absolute top-0 flex flex-col items-start justify-end gap-0.5 border-r border-b border-slate-300 bg-slate-50 px-2 py-1 text-[11px] dark:border-slate-700 dark:bg-slate-800"
                style={{
                  left: `${col.start}px`,
                  width: `${col.size}px`,
                  height: `${HEADER_ROW_HEIGHT}px`,
                }}
                title={`${node.projectSlug}/${node.collectionPrefix}/${node.artifactName} · ${node.title}`}
              >
                <span className="truncate font-mono text-slate-500">
                  {node.projectSlug}/{node.collectionPrefix}/{node.artifactName}
                </span>
                <span className="line-clamp-2 text-slate-700 dark:text-slate-200">
                  {node.title}
                </span>
              </div>
            );
          })}
        </div>

        {/* Row headers — vertically virtualized, horizontally
            pinned to the left. */}
        <div
          className="sticky left-0 z-20"
          style={{
            top: `${HEADER_ROW_HEIGHT}px`,
            width: `${HEADER_COLUMN_WIDTH}px`,
          }}
        >
          {rowVirtualizer.getVirtualItems().map((row) => {
            const node = rows[row.index];
            return (
              <div
                key={row.key}
                data-testid={`matrix-row-header-${node.artifactName}`}
                className="absolute left-0 flex items-center gap-1 overflow-hidden border-r border-b border-slate-300 bg-slate-50 px-2 py-1 text-xs dark:border-slate-700 dark:bg-slate-800"
                style={{
                  top: `${row.start}px`,
                  width: `${HEADER_COLUMN_WIDTH}px`,
                  height: `${row.size}px`,
                }}
                title={`${node.projectSlug}/${node.collectionPrefix}/${node.artifactName} · ${node.title}`}
              >
                <span className="shrink-0 font-mono text-slate-500">
                  {node.projectSlug}/{node.collectionPrefix}/{node.artifactName}
                </span>
                <span className="truncate text-slate-700 dark:text-slate-200">
                  {node.title}
                </span>
              </div>
            );
          })}
        </div>

        {/* Cell grid itself. */}
        {rowVirtualizer.getVirtualItems().map((row) =>
          columnVirtualizer.getVirtualItems().map((col) => {
            const rowNode = rows[row.index];
            const colNode = columns[col.index];
            const selfLink = rowNode.uuid === colNode.uuid;
            const filled = edgeSet.has(edgeKey(rowNode.uuid, colNode.uuid));
            return (
              <MatrixCell
                key={`${row.key}-${col.key}`}
                left={HEADER_COLUMN_WIDTH + col.start}
                top={HEADER_ROW_HEIGHT + row.start}
                width={col.size}
                height={row.size}
                row={rowNode}
                column={colNode}
                filled={filled}
                selfLink={selfLink}
                linkTypeName={linkTypeName}
                onClick={onCellClick}
              />
            );
          }),
        )}
      </div>
    </div>
  );
}

interface CellProps {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
  readonly row: MatrixNodeDto;
  readonly column: MatrixNodeDto;
  readonly filled: boolean;
  readonly selfLink: boolean;
  readonly linkTypeName: string;
  readonly onClick?: (row: MatrixNodeDto, column: MatrixNodeDto) => void;
}

function MatrixCell({
  left,
  top,
  width,
  height,
  row,
  column,
  filled,
  selfLink,
  linkTypeName,
  onClick,
}: CellProps) {
  const testid = `matrix-cell-${row.artifactName}-${column.artifactName}`;
  const baseClass =
    "absolute flex items-center justify-center border-r border-b border-slate-200 text-xs dark:border-slate-800";
  if (selfLink) {
    return (
      <div
        data-testid={testid}
        data-cell-state="self"
        aria-label={`${row.artifactName} is both row and column — self-links are not supported`}
        className={`${baseClass} cursor-not-allowed bg-slate-100 dark:bg-slate-800`}
        style={{
          left: `${left}px`,
          top: `${top}px`,
          width: `${width}px`,
          height: `${height}px`,
        }}
      >
        <span className="text-slate-400">—</span>
      </div>
    );
  }
  const title = filled
    ? `${row.artifactName} ${linkTypeName} ${column.artifactName}`
    : `No ${linkTypeName} link from ${row.artifactName} to ${column.artifactName}`;
  const interactive = onClick !== undefined;
  const palette = filled
    ? "bg-sky-100 hover:bg-sky-200 dark:bg-sky-900/40 dark:hover:bg-sky-800/50"
    : "bg-white hover:bg-slate-50 dark:bg-slate-900 dark:hover:bg-slate-800";
  return (
    <button
      type="button"
      data-testid={testid}
      data-cell-state={filled ? "filled" : "empty"}
      onClick={interactive ? () => onClick!(row, column) : undefined}
      disabled={!interactive}
      title={title}
      className={`${baseClass} ${palette} ${interactive ? "cursor-pointer" : "cursor-default"}`}
      style={{
        left: `${left}px`,
        top: `${top}px`,
        width: `${width}px`,
        height: `${height}px`,
      }}
    >
      {filled ? (
        <span
          aria-label="link present"
          className="h-2 w-2 rounded-full bg-sky-600 dark:bg-sky-300"
        />
      ) : null}
    </button>
  );
}
