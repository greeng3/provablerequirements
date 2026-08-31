import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";

import { TestQueryProvider } from "../../test-utils";
import { ProjectPage } from "../ProjectPage";

function stubJson(body: unknown, status = 200) {
  vi.stubGlobal(
    "fetch",
    async () =>
      new Response(JSON.stringify(body), {
        status,
        headers: { "content-type": "application/json" },
      }),
  );
}

function renderPage(slug: string) {
  return render(
    <TestQueryProvider>
      <MemoryRouter initialEntries={[`/projects/${slug}`]}>
        <Routes>
          <Route path="/projects/:slug" element={<ProjectPage />} />
        </Routes>
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ProjectPage", () => {
  it("renders project details with collection cards linking to each collection", async () => {
    stubJson({
      slug: "sample",
      name: "Sample Project",
      description: "Demo fixture",
      artifactsPath: "artifacts",
      collections: [
        {
          prefix: "REQ",
          name: "Requirements",
          description: "Functional requirements",
          artifactCount: 3,
          expectsCodeTrace: true,
        },
        {
          prefix: "DES",
          name: "Designs",
          description: null,
          artifactCount: 0,
          expectsCodeTrace: true,
        },
      ],
    });

    renderPage("sample");

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "Sample Project" }),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText(/Demo fixture/)).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: /REQ.*Requirements/ }),
    ).toHaveAttribute("href", "/projects/sample/collections/REQ");
    expect(screen.getByRole("link", { name: /DES.*Designs/ })).toHaveAttribute(
      "href",
      "/projects/sample/collections/DES",
    );
  });

  it("shows an empty state when the project has no collections", async () => {
    stubJson({
      slug: "empty",
      name: "Empty Project",
      description: null,
      artifactsPath: "artifacts",
      collections: [],
    });

    renderPage("empty");

    await waitFor(() =>
      expect(screen.getByText(/No Collections yet/)).toBeInTheDocument(),
    );
  });

  it("shows an alert when the project cannot be loaded", async () => {
    stubJson({ error: "project 'nope' not found" }, 404);
    renderPage("nope");
    await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
  });
});
