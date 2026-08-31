import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { CoverageMatrixReportPage } from "../CoverageMatrixReportPage";

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
      <MemoryRouter initialEntries={["/reports/coverage-matrix"]}>
        <CoverageMatrixReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("CoverageMatrixReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders parent rows with a gap badge when no covering children exist", async () => {
    stubFetchByPath({
      "/api/reports/coverage-matrix": {
        kind: "coverage-matrix",
        scope: { kind: "system" },
        coveringLinkTypes: ["satisfies", "verifies"],
        unknownRequestedTypes: [],
        totalParents: 2,
        gapCount: 1,
        parents: [
          {
            parent: {
              uuid: "aaaa",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-a",
              title: "A",
              shape: "content",
              active: true,
            },
            hasGap: false,
            coveringChildren: [
              {
                child: {
                  uuid: "bbbb",
                  projectSlug: "sample",
                  collectionPrefix: "DES",
                  artifactName: "DES-a",
                  title: "Design A",
                  shape: "content",
                  active: true,
                },
                linkType: "satisfies",
              },
            ],
          },
          {
            parent: {
              uuid: "cccc",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-b",
              title: "B",
              shape: "content",
              active: true,
            },
            hasGap: true,
            coveringChildren: [],
          },
        ],
      },
      "/api/link-types": [
        {
          name: "satisfies",
          inverseName: "satisfied-by",
          directed: true,
          acyclic: false,
          source: "builtin",
        },
        {
          name: "verifies",
          inverseName: "verified-by",
          directed: true,
          acyclic: false,
          source: "builtin",
        },
      ],
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText("sample/REQ/REQ-a")).toBeInTheDocument(),
    );
    expect(screen.getByText("sample/REQ/REQ-b")).toBeInTheDocument();
    expect(screen.getByText("sample/DES/DES-a")).toBeInTheDocument();
    expect(screen.getByText("gap")).toBeInTheDocument();
    expect(screen.getByText("covered")).toBeInTheDocument();
  });

  it("surfaces unknown requested link types in a warning banner", async () => {
    stubFetchByPath({
      "/api/reports/coverage-matrix": {
        kind: "coverage-matrix",
        scope: { kind: "system" },
        coveringLinkTypes: ["satisfies"],
        unknownRequestedTypes: ["bogus"],
        totalParents: 0,
        gapCount: 0,
        parents: [],
      },
      "/api/link-types": [],
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/bogus/i),
    );
  });

  it("renders a (+N code) badge + location list when Phase 9b code evidence is present", async () => {
    stubFetchByPath({
      "/api/reports/coverage-matrix": {
        kind: "coverage-matrix",
        scope: { kind: "system" },
        coveringLinkTypes: ["satisfies", "verifies"],
        unknownRequestedTypes: [],
        totalParents: 1,
        gapCount: 0,
        parents: [
          {
            parent: {
              uuid: "aaaa",
              projectSlug: "sample",
              collectionPrefix: "REQ",
              artifactName: "REQ-a",
              title: "A",
              shape: "content",
              active: true,
            },
            hasGap: false,
            coveringChildren: [],
            coveringCodeEvidence: [
              { file: "src/lib.rs", line: 12, verb: "Satisfies" },
              { file: "tests/smoke.rs", line: 4, verb: "Verifies" },
            ],
          },
        ],
      },
      "/api/link-types": [],
      "/api/projects": [],
    });
    renderPage();
    const badge = await screen.findByTestId("coverage-code-badge-REQ-a");
    expect(badge).toHaveTextContent(/\+2 code/);
    // Verb + file:line rows appear inside the expanded list.
    const list = screen.getByTestId("coverage-code-list-REQ-a");
    expect(list).toHaveTextContent(/Satisfies/);
    expect(list).toHaveTextContent(/src\/lib\.rs:12/);
  });
});
