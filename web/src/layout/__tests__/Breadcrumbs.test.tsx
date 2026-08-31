import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";

import { Breadcrumbs } from "../Breadcrumbs";

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/" element={<Breadcrumbs />} />
        <Route path="/projects/:slug" element={<Breadcrumbs />} />
        <Route
          path="/projects/:slug/collections/:prefix"
          element={<Breadcrumbs />}
        />
        <Route
          path="/projects/:slug/collections/:prefix/artifacts/:name"
          element={<Breadcrumbs />}
        />
        <Route path="/artifacts/:uuid" element={<Breadcrumbs />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("Breadcrumbs", () => {
  it("shows only System on the home route", () => {
    renderAt("/");
    const items = screen.getAllByRole("listitem");
    // Filter out the `/` separator items.
    const labels = items
      .filter((el) => !el.hasAttribute("aria-hidden"))
      .map((el) => el.textContent);
    expect(labels).toEqual(["System"]);
  });

  it("shows System / slug on a project route with project not linkable", () => {
    renderAt("/projects/sample");
    const links = screen.getAllByRole("link").map((a) => a.textContent);
    expect(links).toContain("System");
    expect(links).not.toContain("sample");
    // The current crumb is rendered as plain text.
    expect(screen.getByText("sample")).toBeInTheDocument();
  });

  it("links intermediate crumbs on deeper routes", () => {
    renderAt("/projects/sample/collections/REQ/artifacts/REQ-hello");
    const links = screen.getAllByRole("link").map((a) => a.textContent);
    expect(links).toContain("System");
    expect(links).toContain("sample");
    expect(links).toContain("REQ");
    // The terminal crumb is the artifact name as plain text.
    expect(screen.getByText("REQ-hello")).toBeInTheDocument();
  });

  it("uses a short artifact-UUID label for direct /artifacts/:uuid links", () => {
    renderAt("/artifacts/0194f6d0-0001-7000-8000-000000000001");
    expect(screen.getByText(/Artifact 0194f6d0/)).toBeInTheDocument();
  });
});
