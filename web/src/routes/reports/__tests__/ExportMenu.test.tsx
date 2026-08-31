import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ExportMenu } from "../ExportMenu";

describe("ExportMenu", () => {
  it("renders three download links pointing at the export endpoint", () => {
    render(
      <ExportMenu
        kind="unresolved-links"
        scope="system"
        includeInactive={false}
      />,
    );
    expect(
      screen.getByRole("link", { name: "JSON" }).getAttribute("href"),
    ).toBe("/api/reports/unresolved-links/export/json");
    expect(screen.getByRole("link", { name: "CSV" }).getAttribute("href")).toBe(
      "/api/reports/unresolved-links/export/csv",
    );
    expect(
      screen.getByRole("link", { name: "HTML" }).getAttribute("href"),
    ).toBe("/api/reports/unresolved-links/export/html");
  });

  it("marks every link as downloadable so the browser keeps the filename", () => {
    render(
      <ExportMenu kind="link-orphans" scope="system" includeInactive={false} />,
    );
    for (const label of ["JSON", "CSV", "HTML"]) {
      const link = screen.getByRole("link", { name: label });
      expect(link.hasAttribute("download")).toBe(true);
    }
  });

  it("threads scope, includeInactive, and extras into each link URL", () => {
    render(
      <ExportMenu
        kind="coverage-matrix"
        scope="collection:sample/REQ"
        includeInactive={true}
        extra={{ coveringLinkTypes: "satisfies,verifies" }}
      />,
    );
    const href =
      screen.getByRole("link", { name: "CSV" }).getAttribute("href") ?? "";
    const url = new URL(href, "http://x");
    expect(url.searchParams.get("scope")).toBe("collection:sample/REQ");
    expect(url.searchParams.get("includeInactive")).toBe("true");
    expect(url.searchParams.get("coveringLinkTypes")).toBe(
      "satisfies,verifies",
    );
  });

  it("omits the scope param for the default system scope and skips empty extras", () => {
    render(
      <ExportMenu
        kind="impact-analysis"
        scope="system"
        includeInactive={false}
        extra={{ seed: undefined, direction: "dependents" }}
      />,
    );
    const href =
      screen.getByRole("link", { name: "JSON" }).getAttribute("href") ?? "";
    expect(href).not.toContain("scope=");
    expect(href).not.toContain("includeInactive=");
    expect(href).not.toContain("seed=");
    expect(href).toContain("direction=dependents");
  });

  it("renders a disabled pseudo-link for CSV on the cycles report", () => {
    render(<ExportMenu kind="cycles" scope="system" includeInactive={false} />);
    // JSON + HTML are still real links; CSV is the aria-disabled button.
    expect(screen.getByRole("link", { name: "JSON" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "HTML" })).toBeInTheDocument();
    const csv = screen.getByRole("button", { name: "CSV" });
    expect(csv.getAttribute("aria-disabled")).toBe("true");
    expect(csv.getAttribute("title")).toMatch(/flat csv encoding/i);
  });
});
