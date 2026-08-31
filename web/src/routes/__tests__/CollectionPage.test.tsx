import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";

import { TestQueryProvider } from "../../test-utils";
import { CollectionPage } from "../CollectionPage";

function stubFetchByPath(
  responses: Record<string, unknown>,
  fallback?: unknown,
) {
  vi.stubGlobal("fetch", async (input: RequestInfo) => {
    const url = typeof input === "string" ? input : input.toString();
    const match = Object.keys(responses).find((path) => url.endsWith(path));
    const body = match !== undefined ? responses[match] : (fallback ?? []);
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  });
}

function renderPage(slug: string, prefix: string) {
  return render(
    <TestQueryProvider>
      <MemoryRouter
        initialEntries={[`/projects/${slug}/collections/${prefix}`]}
      >
        <Routes>
          <Route
            path="/projects/:slug/collections/:prefix"
            element={<CollectionPage />}
          />
        </Routes>
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

// jsdom reports zero dimensions, which can short-circuit the
// virtualiser. Patch the scroll-element measurements so at least
// the first screen of rows renders.
beforeEach(() => {
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    value: 600,
  });
  Object.defineProperty(HTMLElement.prototype, "clientWidth", {
    configurable: true,
    value: 800,
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("CollectionPage", () => {
  it("renders the collection header and virtualised artifact list", async () => {
    stubFetchByPath({
      "/api/projects/sample/collections/REQ": {
        prefix: "REQ",
        name: "Requirements",
        description: "Functional requirements",
        artifactCount: 2,
        expectsCodeTrace: true,
      },
      "/api/projects/sample/collections/REQ/artifacts": [
        {
          name: "REQ-a",
          uuid: "0194f6d0-0001-7000-8000-000000000001",
          title: "Alpha",
          shape: "content",
          active: true,
        },
        {
          name: "REQ-b",
          uuid: "0194f6d0-0001-7000-8000-000000000002",
          title: "Beta",
          shape: "content",
          active: true,
        },
      ],
    });

    renderPage("sample", "REQ");

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: /REQ.*Requirements/ }),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText(/Functional requirements/)).toBeInTheDocument();

    // Virtualised rows: headings are dynamic, so look for the artifact
    // names.
    await waitFor(() => {
      expect(screen.getByText("REQ-a")).toBeInTheDocument();
    });
    expect(screen.getByText("REQ-b")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /REQ-a.*Alpha/ })).toHaveAttribute(
      "href",
      "/projects/sample/collections/REQ/artifacts/REQ-a",
    );
  });

  it("shows the empty-artifacts state when a collection has no artifacts", async () => {
    stubFetchByPath({
      "/api/projects/sample/collections/REQ": {
        prefix: "REQ",
        name: "Requirements",
        description: null,
        artifactCount: 0,
        expectsCodeTrace: true,
      },
      "/api/projects/sample/collections/REQ/artifacts": [],
    });

    renderPage("sample", "REQ");

    await waitFor(() =>
      expect(screen.getByText(/No artifacts yet/)).toBeInTheDocument(),
    );
  });

  it("flags inactive artifacts in the list", async () => {
    stubFetchByPath({
      "/api/projects/sample/collections/REQ": {
        prefix: "REQ",
        name: "Requirements",
        description: null,
        artifactCount: 1,
        expectsCodeTrace: true,
      },
      "/api/projects/sample/collections/REQ/artifacts": [
        {
          name: "REQ-retired",
          uuid: "0194f6d0-0001-7000-8000-000000000001",
          title: "Retired",
          shape: "content",
          active: false,
        },
      ],
    });

    renderPage("sample", "REQ");

    await waitFor(() =>
      expect(screen.getByText("REQ-retired")).toBeInTheDocument(),
    );
    expect(screen.getByText("inactive")).toBeInTheDocument();
  });
});
