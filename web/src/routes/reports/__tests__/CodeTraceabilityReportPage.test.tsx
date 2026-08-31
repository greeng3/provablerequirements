import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { CodeTraceabilityReportPage } from "../CodeTraceabilityReportPage";

function stubFetchByPath(responses: Record<string, unknown>) {
  const ordered = Object.keys(responses).sort((a, b) => b.length - a.length);
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const match = ordered.find((path) => {
      const stripped = url.split("?")[0] ?? url;
      return stripped.endsWith(path);
    });
    if (match === undefined) {
      return new Response(JSON.stringify({}), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    return new Response(JSON.stringify(responses[match]), {
      status: init?.method === "PUT" ? 204 : 200,
      headers: { "content-type": "application/json" },
    });
  });
}

function renderPage() {
  return render(
    <TestQueryProvider>
      <MemoryRouter initialEntries={["/reports/code-traceability"]}>
        <CodeTraceabilityReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("CodeTraceabilityReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders entries, uncovered gap badge, orphan list, and summary counters", async () => {
    stubFetchByPath({
      "/api/reports/code-traceability": {
        kind: "code-traceability",
        scope: { kind: "system" },
        totalArtifacts: 2,
        uncoveredCount: 1,
        orphanTagCount: 1,
        entries: [
          {
            artifact: {
              uuid: "aaaa",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-covered",
              title: "A covered requirement",
              shape: "content",
              active: true,
            },
            expectsCodeTrace: true,
            hasGap: false,
            locationsByVerb: {
              Satisfies: [{ file: "src/lib.rs", line: 12 }],
              Verifies: [{ file: "tests/smoke.rs", line: 4 }],
            },
          },
          {
            artifact: {
              uuid: "bbbb",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-uncovered",
              title: "Awaits implementation",
              shape: "content",
              active: true,
            },
            expectsCodeTrace: true,
            hasGap: true,
            locationsByVerb: {},
          },
        ],
        orphanTags: [
          {
            file: "src/lib.rs",
            line: 22,
            verb: "Verifies",
            rawId: "REQ-ghost",
          },
        ],
      },
      "/api/link-types": [],
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByTestId("code-trace-summary")).toBeInTheDocument(),
    );
    // Both artifacts listed.
    expect(
      screen.getByTestId("code-trace-entry-REQ-covered"),
    ).toBeInTheDocument();
    const uncovered = screen.getByTestId("code-trace-entry-REQ-uncovered");
    expect(uncovered).toHaveTextContent(/gap/i);
    // Summary counts reach the copy.
    const summary = screen.getByTestId("code-trace-summary");
    expect(summary).toHaveTextContent(/1 uncovered/);
    expect(summary).toHaveTextContent(/1 orphan tag/);
    // Orphan section present.
    expect(screen.getByText(/Orphan tags \(1\)/)).toBeInTheDocument();
    expect(screen.getByText(/REQ-ghost/)).toBeInTheDocument();
  });

  it("shows a friendly empty state when no artifacts are in scope", async () => {
    stubFetchByPath({
      "/api/reports/code-traceability": {
        kind: "code-traceability",
        scope: { kind: "system" },
        totalArtifacts: 0,
        uncoveredCount: 0,
        orphanTagCount: 0,
        entries: [],
        orphanTags: [],
      },
      "/api/link-types": [],
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/no artifacts in scope/i)).toBeInTheDocument(),
    );
  });

  it("hides the orphan-tag section when there are no orphans", async () => {
    stubFetchByPath({
      "/api/reports/code-traceability": {
        kind: "code-traceability",
        scope: { kind: "system" },
        totalArtifacts: 1,
        uncoveredCount: 0,
        orphanTagCount: 0,
        entries: [
          {
            artifact: {
              uuid: "aaaa",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-a",
              title: "A",
              shape: "content",
              active: true,
            },
            expectsCodeTrace: true,
            hasGap: false,
            locationsByVerb: {
              Satisfies: [{ file: "src/lib.rs", line: 1 }],
            },
          },
        ],
        orphanTags: [],
      },
      "/api/link-types": [],
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByTestId("code-trace-entry-REQ-a")).toBeInTheDocument(),
    );
    expect(screen.queryByText(/Orphan tags/)).not.toBeInTheDocument();
  });
});
