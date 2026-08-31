import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";

import { TestQueryProvider } from "../../../../test-utils";
import { GraphPage } from "../GraphPage";

// @xyflow/react pulls in DOM measurement APIs jsdom can't
// satisfy; the page is thin enough that swapping it for a
// pass-through stub keeps the test focused on the filter / banner /
// click-through wiring we own. The stub also surfaces two extra
// buttons per pair (`connect-<a>-<b>`, `selflink-<a>`) so tests
// can drive the drag-to-link flow without simulating pointer
// events through React Flow's handle DOM.
vi.mock("@xyflow/react", () => {
  type StubNode = { id: string; data: { dto?: { artifactName?: string } } };
  type StubProps = {
    nodes?: StubNode[];
    edges?: unknown[];
    onNodeClick?: (event: unknown, node: StubNode) => void;
    onConnect?: (conn: {
      source: string | null;
      target: string | null;
    }) => void;
    children?: React.ReactNode;
  };
  return {
    ReactFlow: ({
      nodes,
      edges,
      onNodeClick,
      onConnect,
      children,
    }: StubProps) => (
      <div data-testid="reactflow-stub">
        <span data-testid="reactflow-node-count">{nodes?.length ?? 0}</span>
        <span data-testid="reactflow-edge-count">{edges?.length ?? 0}</span>
        <ul>
          {(nodes ?? []).map((n) => (
            <li key={n.id}>
              <button
                type="button"
                onClick={() => onNodeClick?.({}, n)}
                data-testid={`node-${n.data?.dto?.artifactName ?? n.id}`}
              >
                {n.data?.dto?.artifactName ?? n.id}
              </button>
              {(nodes ?? [])
                .filter((other) => other.id !== n.id)
                .map((other) => (
                  <button
                    key={other.id}
                    type="button"
                    onClick={() =>
                      onConnect?.({ source: n.id, target: other.id })
                    }
                    data-testid={`connect-${n.id}-${other.id}`}
                  >
                    connect {n.id} → {other.id}
                  </button>
                ))}
              <button
                type="button"
                onClick={() => onConnect?.({ source: n.id, target: n.id })}
                data-testid={`selflink-${n.id}`}
              >
                self-link {n.id}
              </button>
            </li>
          ))}
        </ul>
        {children}
      </div>
    ),
    Background: () => <div />,
    Controls: () => <div />,
    MarkerType: { ArrowClosed: "arrowclosed" },
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
      <MemoryRouter initialEntries={["/explore/graph"]}>
        <GraphPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("GraphPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders node count, edge count, and labels from /api/graph", async () => {
    stubFetchByPath({
      "/api/graph": {
        scope: { kind: "system" },
        totalNodes: 2,
        truncated: false,
        hintAllEdgesAcyclic: true,
        referencedLinkTypes: [
          {
            name: "derives-from",
            inverseName: "derived-into",
            directed: true,
            acyclic: true,
          },
        ],
        nodes: [
          {
            uuid: "a",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: ["core"],
          },
          {
            uuid: "b",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-b",
            title: "B",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
          },
        ],
        edges: [
          {
            sourceUuid: "a",
            targetUuid: "b",
            linkType: "derives-from",
            acyclic: true,
            directed: true,
          },
        ],
      },
      "/api/link-types": [
        {
          name: "derives-from",
          inverseName: "derived-into",
          directed: true,
          acyclic: true,
          source: "builtin",
        },
      ],
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByTestId("reactflow-node-count")).toHaveTextContent("2"),
    );
    expect(screen.getByTestId("reactflow-edge-count")).toHaveTextContent("1");
    expect(screen.getByTestId("node-REQ-a")).toBeInTheDocument();
    expect(screen.getByTestId("node-REQ-b")).toBeInTheDocument();
    // Truncation banner is absent when truncated=false.
    expect(
      screen.queryByTestId("graph-truncation-banner"),
    ).not.toBeInTheDocument();
  });

  it("renders the truncation banner when the server reports overflow", async () => {
    stubFetchByPath({
      "/api/graph": {
        scope: { kind: "system" },
        totalNodes: 725,
        truncated: true,
        hintAllEdgesAcyclic: false,
        referencedLinkTypes: [],
        nodes: [
          {
            uuid: "a",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
          },
        ],
        edges: [],
      },
      "/api/link-types": [],
      "/api/projects": [],
    });
    renderPage();
    const banner = await screen.findByTestId("graph-truncation-banner");
    expect(banner).toHaveTextContent(/500 of 725/);
    expect(banner).toHaveTextContent(/apply filters/i);
  });

  it("navigates to the artifact page when a node is clicked", async () => {
    stubFetchByPath({
      "/api/graph": {
        scope: { kind: "system" },
        totalNodes: 1,
        truncated: false,
        hintAllEdgesAcyclic: false,
        referencedLinkTypes: [],
        nodes: [
          {
            uuid: "a",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
          },
        ],
        edges: [],
      },
      "/api/link-types": [],
      "/api/projects": [],
    });
    function LocationProbe() {
      const location = useLocation();
      return <div data-testid="current-path">{location.pathname}</div>;
    }
    render(
      <TestQueryProvider>
        <MemoryRouter initialEntries={["/explore/graph"]}>
          <Routes>
            <Route path="/explore/graph" element={<GraphPage />} />
            <Route
              path="/projects/:slug/collections/:prefix/artifacts/:name"
              element={<LocationProbe />}
            />
          </Routes>
        </MemoryRouter>
      </TestQueryProvider>,
    );
    const nodeButton = await screen.findByTestId("node-REQ-a");
    await userEvent.click(nodeButton);
    const probe = await screen.findByTestId("current-path");
    expect(probe.textContent).toBe(
      "/projects/sample/collections/REQ/artifacts/REQ-a",
    );
  });

  it("rejects self-link drags with a toast and leaves the dialog closed", async () => {
    stubFetchByPath({
      "/api/graph": {
        scope: { kind: "system" },
        totalNodes: 1,
        truncated: false,
        hintAllEdgesAcyclic: false,
        referencedLinkTypes: [],
        nodes: [
          {
            uuid: "a",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
          },
        ],
        edges: [],
      },
      "/api/link-types": [],
      "/api/projects": [],
    });
    renderPage();
    await screen.findByTestId("node-REQ-a");
    await userEvent.click(screen.getByTestId("selflink-a"));
    const toast = await screen.findByTestId("graph-toast");
    expect(toast).toHaveTextContent(/self-links aren't supported/i);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("opens the link-create dialog when the user drags between two nodes", async () => {
    stubFetchByPath({
      "/api/graph": {
        scope: { kind: "system" },
        totalNodes: 2,
        truncated: false,
        hintAllEdgesAcyclic: false,
        referencedLinkTypes: [],
        nodes: [
          {
            uuid: "a",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-a",
            title: "A",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
          },
          {
            uuid: "b",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            artifactName: "REQ-b",
            title: "B",
            shape: "content",
            active: true,
            derived: false,
            tags: [],
          },
        ],
        edges: [],
      },
      "/api/link-types": [
        {
          name: "derives-from",
          inverseName: "derived-into",
          directed: true,
          acyclic: true,
          source: "builtin",
        },
      ],
      "/api/projects": [],
      "/api/artifacts/a": {
        name: "REQ-a",
        projectSlug: "sample",
        collectionPrefix: "REQ",
        uuid: "a",
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
    await screen.findByTestId("node-REQ-a");
    await userEvent.click(screen.getByTestId("connect-a-b"));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent(/link artifacts/i);
    expect(dialog).toHaveTextContent(/REQ-a/);
    expect(dialog).toHaveTextContent(/REQ-b/);
    // Cancel keeps the page intact.
    await userEvent.click(
      await screen.findByRole("button", { name: /cancel/i }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
  });
});
