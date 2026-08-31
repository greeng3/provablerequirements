import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { CyclesReportPage } from "../CyclesReportPage";

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
      <MemoryRouter initialEntries={["/reports/cycles"]}>
        <CyclesReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("CyclesReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("shows the empty-state copy and the checked-types list", async () => {
    stubFetchByPath({
      "/api/reports/cycles": {
        kind: "cycles",
        scope: { kind: "system" },
        linkTypesChecked: ["derives-from", "supersedes"],
        totalCycles: 0,
        truncated: false,
        cycles: [],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/no cycles in scope/i)).toBeInTheDocument(),
    );
    expect(screen.getByText("derives-from")).toBeInTheDocument();
    expect(screen.getByText("supersedes")).toBeInTheDocument();
  });

  it("renders each cycle with the link-type label and per-node breadcrumbs", async () => {
    stubFetchByPath({
      "/api/reports/cycles": {
        kind: "cycles",
        scope: { kind: "system" },
        linkTypesChecked: ["derives-from"],
        totalCycles: 1,
        truncated: false,
        cycles: [
          {
            linkType: "derives-from",
            nodes: [
              {
                uuid: "aaaa",
                projectSlug: "sample",
                collectionPrefix: "REQ",
                artifactName: "REQ-a",
                title: "A",
                shape: "content",
                active: true,
              },
              {
                uuid: "bbbb",
                projectSlug: "sample",
                collectionPrefix: "REQ",
                artifactName: "REQ-b",
                title: "B",
                shape: "content",
                active: true,
              },
            ],
          },
        ],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getAllByText("sample/REQ/REQ-a").length,
      ).toBeGreaterThanOrEqual(1),
    );
    // REQ-b appears once in the chain.
    expect(screen.getAllByText("sample/REQ/REQ-b").length).toBe(1);
  });

  it("surfaces a truncated banner when the cap is hit", async () => {
    stubFetchByPath({
      "/api/reports/cycles": {
        kind: "cycles",
        scope: { kind: "system" },
        linkTypesChecked: ["derives-from"],
        totalCycles: 1,
        truncated: true,
        cycles: [
          {
            linkType: "derives-from",
            nodes: [
              {
                uuid: "aaaa",
                projectSlug: "sample",
                collectionPrefix: "REQ",
                artifactName: "REQ-a",
                title: "A",
                shape: "content",
                active: true,
              },
            ],
          },
        ],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/truncated/i),
    );
  });
});
