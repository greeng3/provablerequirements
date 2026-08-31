import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { ReportsIndexPage } from "../ReportsIndexPage";

describe("ReportsIndexPage", () => {
  it("lists every planned report kind with a link", () => {
    render(
      <MemoryRouter>
        <ReportsIndexPage />
      </MemoryRouter>,
    );
    for (const title of [
      "Unresolved links",
      "Link-graph orphans",
      "Cycles",
      "Conflicts",
      "Coverage matrix",
      "Impact analysis",
      "Review status",
      "Filesystem orphans",
    ]) {
      expect(screen.getByRole("heading", { name: title })).toBeInTheDocument();
    }
    expect(
      screen
        .getByRole("link", { name: /unresolved links/i })
        .getAttribute("href"),
    ).toBe("/reports/unresolved-links");
  });

  it("has no 'upcoming' badges once every report kind is live", () => {
    render(
      <MemoryRouter>
        <ReportsIndexPage />
      </MemoryRouter>,
    );
    // All eight kinds are live as of Phase 6a.4.
    expect(screen.queryByText("upcoming")).toBeNull();
  });
});
