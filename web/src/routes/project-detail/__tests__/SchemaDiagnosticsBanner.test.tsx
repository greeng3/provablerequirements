import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SchemaDiagnosticsBanner } from "../SchemaDiagnosticsBanner";

describe("SchemaDiagnosticsBanner", () => {
  it("renders nothing when the diagnostics list is empty", () => {
    const { container } = render(<SchemaDiagnosticsBanner diagnostics={[]} />);
    expect(container.innerHTML).toBe("");
  });

  it("surfaces each diagnostic with path, file type, and versions", () => {
    render(
      <SchemaDiagnosticsBanner
        diagnostics={[
          {
            path: "/p/artifacts/req/REQ-future.md",
            fileType: "artifact",
            foundVersion: 99,
            currentVersion: 1,
          },
          {
            path: "/p/artifacts/req/.collection.json",
            fileType: "collection",
            foundVersion: 42,
            currentVersion: 1,
          },
        ]}
      />,
    );
    const banner = screen.getByTestId("schema-diagnostics-banner");
    expect(banner).toHaveTextContent(
      "2 files were written by a newer ReqForge",
    );
    expect(banner).toHaveTextContent("REQ-future.md");
    expect(banner).toHaveTextContent(".collection.json");
    expect(banner).toHaveTextContent("v99");
    expect(banner).toHaveTextContent("v42");
  });

  it("uses singular copy when only one diagnostic is present", () => {
    render(
      <SchemaDiagnosticsBanner
        diagnostics={[
          {
            path: "/p/reqforge.json",
            fileType: "project",
            foundVersion: 5,
            currentVersion: 1,
          },
        ]}
      />,
    );
    expect(
      screen.getByText(/One file was written by a newer ReqForge/i),
    ).toBeInTheDocument();
  });
});
