import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { UnresolvedLinksReportPage } from "../UnresolvedLinksReportPage";

function stubFetchByPath(responses: Record<string, unknown>) {
  // Longer suffix patterns win so '/api/reports/unresolved-links/config'
  // doesn't accidentally match the report-body stub keyed on
  // '/api/reports/unresolved-links'.
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
      <MemoryRouter initialEntries={["/reports/unresolved-links"]}>
        <UnresolvedLinksReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("UnresolvedLinksReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders the empty-state copy when there are no unresolved links", async () => {
    stubFetchByPath({
      "/api/reports/unresolved-links": {
        kind: "unresolved-links",
        scope: { kind: "system" },
        totalUnresolved: 0,
        entries: [],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByText(/no unresolved links in scope/i),
      ).toBeInTheDocument(),
    );
  });

  it("renders an entry row with source, link type, target hint, and reason pill", async () => {
    stubFetchByPath({
      "/api/reports/unresolved-links": {
        kind: "unresolved-links",
        scope: { kind: "system" },
        totalUnresolved: 1,
        entries: [
          {
            sourceUuid: "aaaa",
            sourceProjectSlug: "sample",
            sourceCollectionPrefix: "REQ",
            sourceArtifactName: "REQ-ghost",
            sourceTitle: "Ghost requirement",
            sourceShape: "content",
            linkType: "derives-from",
            targetUuid: "bbbb",
            targetHintProjectSlug: "sample",
            targetHintCollectionPrefix: "DES",
            targetHintArtifactName: "DES-target",
            reason: "target-missing",
          },
        ],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText("sample/REQ/REQ-ghost")).toBeInTheDocument(),
    );
    expect(screen.getByText("derives-from")).toBeInTheDocument();
    expect(screen.getByText("sample/DES/DES-target")).toBeInTheDocument();
    expect(screen.getByText("target-missing")).toBeInTheDocument();
  });

  it("surfaces the Include-inactive toggle defaulting unchecked", async () => {
    stubFetchByPath({
      "/api/reports/unresolved-links": {
        kind: "unresolved-links",
        scope: { kind: "system" },
        totalUnresolved: 0,
        entries: [],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByLabelText(/include inactive artifacts/i),
      ).toBeInTheDocument(),
    );
    const checkbox = screen.getByLabelText(/include inactive artifacts/i);
    expect(checkbox).not.toBeChecked();
  });
});
