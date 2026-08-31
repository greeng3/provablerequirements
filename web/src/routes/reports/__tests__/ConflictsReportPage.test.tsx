import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { ConflictsReportPage } from "../ConflictsReportPage";

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
      <MemoryRouter initialEntries={["/reports/conflicts"]}>
        <ConflictsReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("ConflictsReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("shows the empty-state copy when there are no conflict pairs", async () => {
    stubFetchByPath({
      "/api/reports/conflicts": {
        kind: "conflicts",
        scope: { kind: "system" },
        totalPairs: 0,
        pairs: [],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByText(/no conflict pairs in scope/i),
      ).toBeInTheDocument(),
    );
  });

  it("renders pair rows with bidirectional / one-sided direction badges", async () => {
    stubFetchByPath({
      "/api/reports/conflicts": {
        kind: "conflicts",
        scope: { kind: "system" },
        totalPairs: 2,
        pairs: [
          {
            first: {
              uuid: "aaaa",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-a",
              title: "A",
              shape: "content",
              active: true,
            },
            second: {
              uuid: "bbbb",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-b",
              title: "B",
              shape: "content",
              active: true,
            },
            bidirectional: true,
          },
          {
            first: {
              uuid: "cccc",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-c",
              title: "C",
              shape: "content",
              active: true,
            },
            second: {
              uuid: "dddd",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-d",
              title: "D",
              shape: "content",
              active: true,
            },
            bidirectional: false,
          },
        ],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText("sample/REQ/REQ-a")).toBeInTheDocument(),
    );
    expect(screen.getByText("sample/REQ/REQ-d")).toBeInTheDocument();
    expect(screen.getByText("bidirectional")).toBeInTheDocument();
    expect(screen.getByText("one-sided")).toBeInTheDocument();
  });
});
