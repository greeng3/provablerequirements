import { Suspense, lazy } from "react";
import { Route, Routes } from "react-router-dom";

import { AppShell } from "../layout/AppShell";
import { ArtifactDiffPage } from "./ArtifactDiffPage";
import { ArtifactPage } from "./ArtifactPage";
import { CollectionPage } from "./CollectionPage";
import { BrowsePage } from "./browse/BrowsePage";
import { CodeTraceabilityReportPage } from "./reports/CodeTraceabilityReportPage";
import { LlmProvidersPage } from "./llm/LlmProvidersPage";
import { ProofPage } from "./proof/ProofPage";
import { SearchPage } from "./search/SearchPage";
import { ProjectPage } from "./ProjectPage";
import { ReportsIndexPage } from "./ReportsIndexPage";
import { ConflictsReportPage } from "./reports/ConflictsReportPage";
import { CoverageMatrixReportPage } from "./reports/CoverageMatrixReportPage";
import { CyclesReportPage } from "./reports/CyclesReportPage";
import { FilesystemOrphansReportPage } from "./reports/FilesystemOrphansReportPage";
import { ImpactAnalysisReportPage } from "./reports/ImpactAnalysisReportPage";
import { LinkOrphansReportPage } from "./reports/LinkOrphansReportPage";
import { ReviewStatusReportPage } from "./reports/ReviewStatusReportPage";
import { UnresolvedLinksReportPage } from "./reports/UnresolvedLinksReportPage";
import { ReviewQueuePage } from "./ReviewQueuePage";
import { HomeRedirect } from "./HomeRedirect";

// The explore views pull in the heavy graph/matrix libraries
// (ReactFlow + dagre + d3-force), which no landing path needs.
// Lazy-load them into their own chunks, each behind a Suspense
// boundary local to its route.
const GraphPage = lazy(() =>
  import("./explore/graph/GraphPage").then((m) => ({ default: m.GraphPage })),
);
const MatrixPage = lazy(() =>
  import("./explore/matrix/MatrixPage").then((m) => ({ default: m.MatrixPage })),
);

const lazyFallback = <p className="text-sm text-slate-500">Loading…</p>;

/// Application routes. Bare URL patterns — the AppShell layout is
/// applied at the outer Route via Outlet.
export function AppRoutes() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<HomeRedirect />} />
        <Route path="/projects/:slug" element={<ProjectPage />} />
        <Route
          path="/projects/:slug/collections/:prefix"
          element={<CollectionPage />}
        />
        <Route
          path="/projects/:slug/collections/:prefix/artifacts/:name"
          element={<ArtifactPage />}
        />
        <Route path="/artifacts/:uuid" element={<ArtifactPage />} />
        <Route path="/artifacts/:uuid/diff" element={<ArtifactDiffPage />} />
        <Route path="/reviews" element={<ReviewQueuePage />} />
        <Route path="/reports" element={<ReportsIndexPage />} />
        <Route
          path="/reports/unresolved-links"
          element={<UnresolvedLinksReportPage />}
        />
        <Route
          path="/reports/link-orphans"
          element={<LinkOrphansReportPage />}
        />
        <Route path="/reports/cycles" element={<CyclesReportPage />} />
        <Route path="/reports/conflicts" element={<ConflictsReportPage />} />
        <Route
          path="/reports/coverage-matrix"
          element={<CoverageMatrixReportPage />}
        />
        <Route
          path="/reports/impact-analysis"
          element={<ImpactAnalysisReportPage />}
        />
        <Route
          path="/reports/review-status"
          element={<ReviewStatusReportPage />}
        />
        <Route
          path="/reports/filesystem-orphans"
          element={<FilesystemOrphansReportPage />}
        />
        <Route
          path="/reports/code-traceability"
          element={<CodeTraceabilityReportPage />}
        />
        <Route path="/proof" element={<ProofPage />} />
        <Route
          path="/explore/graph"
          element={
            <Suspense fallback={lazyFallback}>
              <GraphPage />
            </Suspense>
          }
        />
        <Route
          path="/explore/matrix"
          element={
            <Suspense fallback={lazyFallback}>
              <MatrixPage />
            </Suspense>
          }
        />
        <Route path="/search" element={<SearchPage />} />
        <Route path="/browse" element={<BrowsePage />} />
        <Route path="/llm" element={<LlmProvidersPage />} />
      </Route>
    </Routes>
  );
}
