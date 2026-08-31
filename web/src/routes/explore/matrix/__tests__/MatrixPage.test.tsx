import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../../test-utils";
import { MatrixPage } from "../MatrixPage";

// TanStack Virtual renders nothing in jsdom because it keys
// off a live scroll element's ResizeObserver-driven
// dimensions. We only care about wiring in these tests, so
// swap the hook for a pass-through that mounts every item.
vi.mock("@tanstack/react-virtual", () => {
  type Opts = {
    count: number;
    estimateSize: (i: number) => number;
  };
  return {
    useVirtualizer: (opts: Opts) => {
      const size = opts.estimateSize(0);
      const items = Array.from({ length: opts.count }, (_, i) => ({
        index: i,
        key: String(i),
        start: i * size,
        size,
      }));
      return {
        getVirtualItems: () => items,
        getTotalSize: () => opts.count * size,
      };
    },
  };
});

function stubFetchByPath(
  responses: Record<string, unknown | ((url: string) => unknown)>,
) {
  const ordered = Object.keys(responses).sort((a, b) => b.length - a.length);
  vi.stubGlobal("fetch", async (input: RequestInfo) => {
    const url = typeof input === "string" ? input : input.toString();
    const stripped = url.split("?")[0] ?? url;
    const match = ordered.find((path) => stripped.endsWith(path));
    if (match === undefined) {
      return new Response(JSON.stringify({}), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    const raw = responses[match];
    const body = typeof raw === "function" ? raw(url) : raw;
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  });
}

function renderPage() {
  return render(
    <TestQueryProvider>
      <MemoryRouter initialEntries={["/explore/matrix"]}>
        <MatrixPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

function matrixBody(
  override: Partial<{
    totalRows: number;
    totalColumns: number;
    rowsTruncated: boolean;
    columnsTruncated: boolean;
    rows: unknown[];
    columns: unknown[];
    edges: unknown[];
  }> = {},
) {
  return {
    rowScope: { kind: "system" },
    columnScope: { kind: "system" },
    linkType: {
      name: "satisfies",
      inverseName: "satisfied-by",
      directed: true,
      acyclic: false,
    },
    totalRows: override.totalRows ?? 0,
    rowsTruncated: override.rowsTruncated ?? false,
    totalColumns: override.totalColumns ?? 0,
    columnsTruncated: override.columnsTruncated ?? false,
    rows: override.rows ?? [],
    columns: override.columns ?? [],
    edges: override.edges ?? [],
  };
}

const catalog = [
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
];

describe("MatrixPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders row and column headers from /api/matrix and surfaces a filled cell", async () => {
    stubFetchByPath({
      "/api/matrix": matrixBody({
        totalRows: 2,
        totalColumns: 2,
        rows: [
          {
            uuid: "r1",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
            reviewState: "approved",
          },
          {
            uuid: "r2",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-b",
            title: "B",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
            reviewState: "never-reviewed",
          },
        ],
        columns: [
          {
            uuid: "c1",
            projectSlug: "sample",
            collectionPrefix: "DES",
            artifactName: "DES-a",
            title: "Design A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
            reviewState: "never-reviewed",
          },
          {
            uuid: "c2",
            projectSlug: "sample",
            collectionPrefix: "DES",
            artifactName: "DES-b",
            title: "Design B",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
            reviewState: "never-reviewed",
          },
        ],
        edges: [{ rowUuid: "r1", columnUuid: "c1" }],
      }),
      "/api/link-types": catalog,
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByTestId("matrix-row-header-REQ-a")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("matrix-row-header-REQ-b")).toBeInTheDocument();
    expect(screen.getByTestId("matrix-col-header-DES-a")).toBeInTheDocument();
    expect(screen.getByTestId("matrix-col-header-DES-b")).toBeInTheDocument();
    // Filled cell carries data-cell-state="filled"; empty cells
    // in the same row stay "empty".
    expect(screen.getByTestId("matrix-cell-REQ-a-DES-a")).toHaveAttribute(
      "data-cell-state",
      "filled",
    );
    expect(screen.getByTestId("matrix-cell-REQ-a-DES-b")).toHaveAttribute(
      "data-cell-state",
      "empty",
    );
  });

  it("blocks the grid with the truncation banner when either axis overflows", async () => {
    stubFetchByPath({
      "/api/matrix": matrixBody({
        totalRows: 725,
        rowsTruncated: true,
        totalColumns: 30,
      }),
      "/api/link-types": catalog,
      "/api/projects": [],
    });
    renderPage();
    const banner = await screen.findByTestId("matrix-truncation-banner");
    expect(banner).toHaveTextContent(/row axis has 725 items/i);
    expect(banner).toHaveTextContent(/apply filters to narrow below 500/i);
    // Grid is absent when banner is shown.
    expect(screen.queryByTestId("matrix-grid")).not.toBeInTheDocument();
  });

  it("marks self-link cells as non-interactive when row and column resolve to the same UUID", async () => {
    stubFetchByPath({
      "/api/matrix": matrixBody({
        totalRows: 1,
        totalColumns: 1,
        rows: [
          {
            uuid: "same",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
            reviewState: "approved",
          },
        ],
        columns: [
          {
            uuid: "same",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
            reviewState: "approved",
          },
        ],
        edges: [],
      }),
      "/api/link-types": catalog,
      "/api/projects": [],
    });
    renderPage();
    const cell = await screen.findByTestId("matrix-cell-REQ-a-REQ-a");
    expect(cell).toHaveAttribute("data-cell-state", "self");
  });

  it("opens the MatrixCellDialog when a non-self-link cell is clicked", async () => {
    stubFetchByPath({
      "/api/matrix": matrixBody({
        totalRows: 1,
        totalColumns: 1,
        rows: [
          {
            uuid: "r1",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
            reviewState: "approved",
          },
        ],
        columns: [
          {
            uuid: "c1",
            projectSlug: "sample",
            collectionPrefix: "DES",
            artifactName: "DES-a",
            title: "Design A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
            reviewState: "never-reviewed",
          },
        ],
        edges: [],
      }),
      "/api/link-types": catalog,
      "/api/projects": [],
      "/api/artifacts/r1": {
        name: "REQ-a",
        projectSlug: "sample",
        collectionPrefix: "REQ",
        uuid: "r1",
        title: "A",
        shape: "content",
        description: null,
        active: true,
        derived: false,
        createdAt: "2026-04-22T00:00:00Z",
        modifiedAt: "2026-04-22T00:00:00Z",
        tags: [],
        links: [],
        reviewLog: [],
        reviewState: { state: "none" },
        body: "",
      },
    });
    renderPage();
    const cell = await screen.findByTestId("matrix-cell-REQ-a-DES-a");
    expect(cell).toHaveAttribute("data-cell-state", "empty");
    await userEvent.click(cell);
    const heading = await screen.findByRole("heading", {
      name: /create link/i,
    });
    expect(heading).toBeInTheDocument();
  });

  it("flips linkType via the picker and refetches the matrix", async () => {
    const calls: string[] = [];
    vi.stubGlobal("fetch", async (input: RequestInfo) => {
      const url = typeof input === "string" ? input : input.toString();
      calls.push(url);
      const stripped = url.split("?")[0] ?? url;
      if (stripped.endsWith("/api/matrix")) {
        return new Response(JSON.stringify(matrixBody()), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      if (stripped.endsWith("/api/link-types")) {
        return new Response(JSON.stringify(catalog), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      if (stripped.endsWith("/api/projects")) {
        return new Response(JSON.stringify([]), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      return new Response("{}", {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    });
    renderPage();
    // Wait for link catalog to populate the select options.
    await waitFor(() => {
      expect(
        screen.getByRole("option", { name: /verifies/i }),
      ).toBeInTheDocument();
    });
    await userEvent.selectOptions(
      screen.getByLabelText(/link type/i),
      "verifies",
    );
    await waitFor(() => {
      expect(
        calls.some(
          (u) => u.includes("/api/matrix") && u.includes("linkType=verifies"),
        ),
      ).toBe(true);
    });
  });
});
