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
  it("renders no crumbs on the home route", () => {
    renderAt("/");
    expect(screen.queryAllByRole("listitem")).toHaveLength(0);
  });

  it("renders no crumbs on the bare project route", () => {
    // Single subject: there is no System or project crumb, and the
    // bare project page is itself the root, so nothing precedes it.
    renderAt("/projects/sample");
    expect(screen.queryAllByRole("listitem")).toHaveLength(0);
  });

  it("starts at the collection crumb on a collection route", () => {
    renderAt("/projects/sample/collections/REQ");
    expect(screen.queryByText("sample")).not.toBeInTheDocument();
    // The collection is the current, plain-text (non-linked) crumb.
    expect(screen.getByText("REQ")).toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });

  it("links the collection crumb on a deeper artifact route", () => {
    renderAt("/projects/sample/collections/REQ/artifacts/REQ-hello");
    const links = screen.getAllByRole("link").map((a) => a.textContent);
    expect(links).toEqual(["REQ"]);
    expect(screen.queryByText("sample")).not.toBeInTheDocument();
    // The terminal crumb is the artifact name as plain text.
    expect(screen.getByText("REQ-hello")).toBeInTheDocument();
  });

  it("uses a short artifact-UUID label for direct /artifacts/:uuid links", () => {
    renderAt("/artifacts/0194f6d0-0001-7000-8000-000000000001");
    expect(screen.getByText(/Artifact 0194f6d0/)).toBeInTheDocument();
  });
});
