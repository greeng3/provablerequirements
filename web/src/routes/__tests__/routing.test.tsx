import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { AppRoutes } from "../";
import { TestQueryProvider } from "../../test-utils";

/// Per-path canned responses. Returns `[]` for unmatched paths, so
/// list endpoints work by default.
function stubByPath(responses: Record<string, unknown>) {
  vi.stubGlobal("fetch", async (input: RequestInfo) => {
    const url = typeof input === "string" ? input : input.toString();
    const match = Object.keys(responses).find((p) => url.endsWith(p));
    return new Response(
      JSON.stringify(match !== undefined ? responses[match] : []),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  });
}

function renderAt(path: string) {
  return render(
    <TestQueryProvider>
      <MemoryRouter initialEntries={[path]}>
        <AppRoutes />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

beforeEach(() => {
  // Default: empty responses for any endpoint. Individual tests
  // override via stubByPath before calling renderAt.
  stubByPath({});
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("AppRoutes", () => {
  it("renders SystemHomePage at /", async () => {
    renderAt("/");
    expect(
      screen.getByRole("heading", { name: /System Home/i }),
    ).toBeInTheDocument();
  });

  it("renders ProjectPage at /projects/:slug", async () => {
    stubByPath({
      "/api/projects/sample-project": {
        slug: "sample-project",
        name: "Sample Project",
        description: null,
        artifactsPath: "artifacts",
        collections: [],
      },
    });
    renderAt("/projects/sample-project");
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "Sample Project" }),
      ).toBeInTheDocument(),
    );
  });

  it("renders CollectionPage at /projects/:slug/collections/:prefix", async () => {
    stubByPath({
      "/api/projects/sample-project/collections/REQ": {
        prefix: "REQ",
        name: "Requirements",
        description: null,
        artifactCount: 0,
        expectsCodeTrace: true,
      },
    });
    renderAt("/projects/sample-project/collections/REQ");
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: /REQ.*Requirements/ }),
      ).toBeInTheDocument(),
    );
  });

  it("renders ArtifactPage at /projects/:slug/collections/:prefix/artifacts/:name", () => {
    // ArtifactPage begins fetching immediately; the "Loading…"
    // marker confirms the route dispatched correctly.
    renderAt("/projects/sample-project/collections/REQ/artifacts/REQ-hello");
    expect(screen.getByText(/Loading artifact/i)).toBeInTheDocument();
  });

  it("renders ArtifactPage at /artifacts/:uuid", () => {
    renderAt("/artifacts/0194f6d0-0001-7000-8000-000000000001");
    expect(screen.getByText(/Loading artifact/i)).toBeInTheDocument();
  });
});
