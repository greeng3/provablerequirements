import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { UnresolvedBadge } from "../UnresolvedBadge";
import type { LinkHint } from "../../../api/types";

const HINT: LinkHint = {
  projectSlug: "other-repo",
  collectionPrefix: "REQ",
  artifactName: "REQ-external",
};

describe("UnresolvedBadge", () => {
  it("renders the informational 'mount <slug>' copy with the hint project", () => {
    render(<UnresolvedBadge hint={HINT} />);
    expect(screen.getByText(/unresolved/i)).toBeInTheDocument();
    expect(screen.getByText("other-repo")).toBeInTheDocument();
  });

  it("offers no init shortcut — there is only one subject", () => {
    render(<UnresolvedBadge hint={HINT} />);
    expect(
      screen.queryByRole("link", { name: /init this mount/i }),
    ).not.toBeInTheDocument();
  });
});
