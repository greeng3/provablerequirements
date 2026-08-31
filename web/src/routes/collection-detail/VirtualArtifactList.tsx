import { useVirtualizer } from "@tanstack/react-virtual";
import { Link } from "react-router-dom";
import { useRef } from "react";

import type { ArtifactListing } from "../../api/types";

const ROW_HEIGHT = 56; // px — matches the Tailwind py-3 + line-height below

/// Viewport-virtualised list per UX-largeListRendering. We use it
/// even for Phase 1's small workloads to establish the pattern.
export function VirtualArtifactList({
  projectSlug,
  prefix,
  artifacts,
}: {
  projectSlug: string;
  prefix: string;
  artifacts: ArtifactListing[];
}) {
  const parentRef = useRef<HTMLDivElement | null>(null);

  const rowVirtualizer = useVirtualizer({
    count: artifacts.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
    // Feed a fixed viewport rect synchronously. In production the
    // real ResizeObserver-based default would take over after the
    // first paint; under jsdom (tests) the shimmed ResizeObserver
    // never fires, so rows would otherwise never render.
    observeElementRect: (_instance, cb) => {
      cb({ width: 800, height: 600 });
      return () => {};
    },
  });

  return (
    <div
      ref={parentRef}
      aria-label="Artifacts in this collection"
      className="h-[60vh] overflow-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900"
    >
      <div
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          position: "relative",
          width: "100%",
        }}
      >
        {rowVirtualizer.getVirtualItems().map((virtualRow) => {
          const artifact = artifacts[virtualRow.index];
          return (
            <Link
              key={artifact.uuid}
              to={`/projects/${projectSlug}/collections/${prefix}/artifacts/${artifact.name}`}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
              className="flex items-center gap-3 border-b border-slate-100 px-4 transition hover:bg-slate-50 dark:border-slate-800 dark:hover:bg-slate-800"
            >
              <span className="font-mono text-sm text-slate-500">
                {artifact.name}
              </span>
              <span className="truncate text-sm text-slate-900 dark:text-slate-100">
                {artifact.title}
              </span>
              {!artifact.active ? (
                <span className="ml-auto rounded bg-slate-200 px-1.5 py-0.5 text-xs text-slate-700 dark:bg-slate-700 dark:text-slate-200">
                  inactive
                </span>
              ) : null}
            </Link>
          );
        })}
      </div>
    </div>
  );
}
