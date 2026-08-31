import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { OutgoingLinks } from "../OutgoingLinks";
import { TestQueryProvider } from "../../../test-utils";
import type { LinkType, LinkView } from "../../../api/types";

const builtin: LinkType = {
  name: "derives-from",
  inverseName: "derived-into",
  directed: true,
  acyclic: true,
  source: "builtin",
};

const resolvedLink: LinkView = {
  targetUuid: "0194f6d0-0001-7000-8000-000000000002",
  type: "derives-from",
  hint: {
    projectSlug: "sample",
    collectionPrefix: "REQ",
    artifactName: "REQ-parent",
  },
  resolution: "resolved",
  typeMetadata: builtin,
  targetSummary: {
    projectSlug: "sample",
    collectionPrefix: "REQ",
    artifactName: "REQ-parent",
    title: "Parent requirement",
  },
};

const unresolvedLink: LinkView = {
  targetUuid: "0194f6d0-0001-7000-8000-000000000099",
  type: "satisfies",
  hint: {
    projectSlug: "other-repo",
    collectionPrefix: "REQ",
    artifactName: "REQ-external",
  },
  resolution: "unresolved",
  typeMetadata: {
    name: "satisfies",
    inverseName: "satisfied-by",
    directed: true,
    acyclic: false,
    source: "builtin",
  },
};

const unknownTypeLink: LinkView = {
  targetUuid: "0194f6d0-0001-7000-8000-000000000003",
  type: "mitigates",
  hint: {
    projectSlug: "sample",
    collectionPrefix: "REQ",
    artifactName: "REQ-child",
  },
  resolution: "unknownType",
};

function renderWithRouter(ui: React.ReactNode) {
  return render(
    <TestQueryProvider>
      <MemoryRouter>{ui}</MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("OutgoingLinks", () => {
  beforeEach(() => {
    // Stub the network so link rows render without a real server.
    vi.spyOn(globalThis, "fetch").mockImplementation(
      async () => new Response(JSON.stringify([]), { status: 200 }),
    );
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });
  it("shows an empty-state message when there are no links", () => {
    renderWithRouter(<OutgoingLinks links={[]} />);
    expect(screen.getByText(/no outgoing links/i)).toBeInTheDocument();
  });

  it("renders resolved links as clickable with the target title", () => {
    renderWithRouter(<OutgoingLinks links={[resolvedLink]} />);
    expect(screen.getByRole("link", { name: /REQ-parent/ })).toHaveAttribute(
      "href",
      `/artifacts/${resolvedLink.targetUuid}`,
    );
    expect(screen.getByText(/Parent requirement/)).toBeInTheDocument();
  });

  it("renders an unresolved link with the 'mount <slug>' affordance", () => {
    renderWithRouter(<OutgoingLinks links={[unresolvedLink]} />);
    expect(screen.getByText(/REQ-external/)).toBeInTheDocument();
    expect(screen.getByText(/unresolved/i)).toBeInTheDocument();
    expect(screen.getByText("other-repo")).toBeInTheDocument();
  });

  it("renders an unknown-type link with the amber indicator", () => {
    renderWithRouter(<OutgoingLinks links={[unknownTypeLink]} />);
    expect(screen.getByText(/unknown link type/i)).toBeInTheDocument();
  });

  it("fires onRemove with the link's index when the remove button clicks", async () => {
    const user = userEvent.setup();
    const onRemove = vi.fn();
    renderWithRouter(
      <OutgoingLinks
        links={[resolvedLink, unresolvedLink]}
        onRemove={onRemove}
      />,
    );
    const removeButtons = screen.getAllByRole("button", { name: /^Remove/ });
    expect(removeButtons).toHaveLength(2);
    await user.click(removeButtons[1]);
    expect(onRemove).toHaveBeenCalledWith(1);
  });

  it("omits remove buttons when onRemove is not provided", () => {
    renderWithRouter(<OutgoingLinks links={[resolvedLink]} />);
    expect(
      screen.queryByRole("button", { name: /^Remove/ }),
    ).not.toBeInTheDocument();
  });
});
