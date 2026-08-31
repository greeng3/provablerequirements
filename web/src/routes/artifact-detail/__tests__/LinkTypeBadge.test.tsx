import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { LinkTypeBadge } from "../LinkTypeBadge";
import type { LinkType } from "../../../api/types";

const satisfies: LinkType = {
  name: "satisfies",
  inverseName: "satisfied-by",
  directed: true,
  acyclic: false,
  source: "builtin",
};

const mitigates: LinkType = {
  name: "mitigates",
  inverseName: "mitigated-by",
  directed: true,
  acyclic: false,
  source: "system",
};

describe("LinkTypeBadge", () => {
  it("renders the type name and describes the metadata in the tooltip", () => {
    render(<LinkTypeBadge typeName="satisfies" metadata={satisfies} />);
    const badge = screen.getByText("satisfies");
    expect(badge).toBeInTheDocument();
    expect(badge.getAttribute("title")).toMatch(/directed/);
    expect(badge.getAttribute("title")).toMatch(/satisfied-by/);
    expect(badge.getAttribute("title")).toMatch(/builtin/);
  });

  it("marks system-declared types with a different source in the tooltip", () => {
    render(<LinkTypeBadge typeName="mitigates" metadata={mitigates} />);
    expect(screen.getByText("mitigates").getAttribute("title")).toMatch(
      /system/,
    );
  });

  it("renders an 'unknown' tooltip when metadata is absent", () => {
    render(<LinkTypeBadge typeName="orphan" />);
    const badge = screen.getByText("orphan");
    expect(badge.getAttribute("title")).toMatch(/[Uu]nknown/);
    expect(badge.getAttribute("aria-label")).toMatch(/unknown/);
  });
});
