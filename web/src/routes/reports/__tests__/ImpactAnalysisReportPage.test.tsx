import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { ImpactAnalysisReportPage } from "../ImpactAnalysisReportPage";

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
      <MemoryRouter initialEntries={["/reports/impact-analysis"]}>
        <ImpactAnalysisReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("ImpactAnalysisReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("shows the missing-seed banner when the backend reports one", async () => {
    stubFetchByPath({
      "/api/reports/impact-analysis": {
        kind: "impact-analysis",
        scope: { kind: "system" },
        direction: "dependents",
        totalImpacted: 0,
        impacted: [],
        missingSeedReason: "Pick a seed artifact to see its transitive impact.",
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/pick a seed artifact/i)).toBeInTheDocument(),
    );
  });

  it("renders depth + link-type columns for each impacted artifact", async () => {
    stubFetchByPath({
      "/api/reports/impact-analysis": {
        kind: "impact-analysis",
        scope: { kind: "system" },
        seed: {
          uuid: "aaaa",
          projectSlug: "sample",
          collectionPrefix: "REQ",
          artifactName: "REQ-a",
          title: "A",
          shape: "content",
          active: true,
        },
        direction: "dependents",
        totalImpacted: 2,
        impacted: [
          {
            node: {
              uuid: "bbbb",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-b",
              title: "B",
              shape: "content",
              active: true,
            },
            depth: 1,
            linkTypes: ["derives-from"],
          },
          {
            node: {
              uuid: "cccc",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-c",
              title: "C",
              shape: "content",
              active: true,
            },
            depth: 2,
            linkTypes: ["derives-from"],
          },
        ],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText("sample/REQ/REQ-b")).toBeInTheDocument(),
    );
    expect(screen.getByText("sample/REQ/REQ-c")).toBeInTheDocument();
    // Depth column + link-type column both populated.
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getAllByText("derives-from").length).toBeGreaterThanOrEqual(
      2,
    );
  });
});
