import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { LinkOrphansReportPage } from "../LinkOrphansReportPage";

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
      <MemoryRouter initialEntries={["/reports/link-orphans"]}>
        <LinkOrphansReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("LinkOrphansReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("shows the empty-state copy when there are no orphans", async () => {
    stubFetchByPath({
      "/api/reports/link-orphans": {
        kind: "link-orphans",
        scope: { kind: "system" },
        totalOrphans: 0,
        entries: [],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByText(/no link-graph orphans in scope/i),
      ).toBeInTheDocument(),
    );
  });

  it("renders orphan rows with project/collection/name + status pills", async () => {
    stubFetchByPath({
      "/api/reports/link-orphans": {
        kind: "link-orphans",
        scope: { kind: "system" },
        totalOrphans: 2,
        entries: [
          {
            uuid: "aaaa",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-solo",
            title: "Solo requirement",
            shape: "content",
            active: true,
            derived: false,
          },
          {
            uuid: "bbbb",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-stale",
            title: "Stale",
            shape: "content",
            active: false,
            derived: true,
          },
        ],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText("sample/REQ/REQ-solo")).toBeInTheDocument(),
    );
    expect(screen.getByText("sample/REQ/REQ-stale")).toBeInTheDocument();
    expect(screen.getByText("inactive")).toBeInTheDocument();
    expect(screen.getByText("derived")).toBeInTheDocument();
  });
});
