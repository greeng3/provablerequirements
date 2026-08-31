import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { ReviewStatusReportPage } from "../ReviewStatusReportPage";

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
      <MemoryRouter initialEntries={["/reports/review-status"]}>
        <ReviewStatusReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("ReviewStatusReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("shows empty-state copy when no artifacts are in scope", async () => {
    stubFetchByPath({
      "/api/reports/review-status": {
        kind: "review-status",
        scope: { kind: "system" },
        totals: {
          approved: 0,
          rejected: 0,
          reRequested: 0,
          neverReviewed: 0,
        },
        byProject: [],
        byCollection: [],
        byShape: {
          content: {
            approved: 0,
            rejected: 0,
            reRequested: 0,
            neverReviewed: 0,
          },
          blob: {
            approved: 0,
            rejected: 0,
            reRequested: 0,
            neverReviewed: 0,
          },
          url: { approved: 0, rejected: 0, reRequested: 0, neverReviewed: 0 },
        },
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/no artifacts in scope/i)).toBeInTheDocument(),
    );
  });

  it("renders totals + per-shape + per-project facet rows", async () => {
    stubFetchByPath({
      "/api/reports/review-status": {
        kind: "review-status",
        scope: { kind: "system" },
        totals: {
          approved: 2,
          rejected: 1,
          reRequested: 0,
          neverReviewed: 3,
        },
        byProject: [
          {
            projectSlug: "sample",
            counts: {
              approved: 2,
              rejected: 1,
              reRequested: 0,
              neverReviewed: 3,
            },
          },
        ],
        byCollection: [
          {
            projectSlug: "sample",
            collectionPrefix: "REQ",
            counts: {
              approved: 2,
              rejected: 1,
              reRequested: 0,
              neverReviewed: 3,
            },
          },
        ],
        byShape: {
          content: {
            approved: 2,
            rejected: 1,
            reRequested: 0,
            neverReviewed: 3,
          },
          blob: {
            approved: 0,
            rejected: 0,
            reRequested: 0,
            neverReviewed: 0,
          },
          url: { approved: 0, rejected: 0, reRequested: 0, neverReviewed: 0 },
        },
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() => expect(screen.getByText("Totals")).toBeInTheDocument());
    expect(screen.getByText("sample")).toBeInTheDocument();
    expect(screen.getByText("sample/REQ")).toBeInTheDocument();
  });
});
