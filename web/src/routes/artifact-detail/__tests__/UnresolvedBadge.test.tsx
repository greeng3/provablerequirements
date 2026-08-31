import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";

import { UnresolvedBadge } from "../UnresolvedBadge";
import { TestQueryProvider } from "../../../test-utils";
import type { LinkHint, MountEntry } from "../../../api/types";

const HINT: LinkHint = {
  projectSlug: "other-repo",
  collectionPrefix: "REQ",
  artifactName: "REQ-external",
};

function installMountsStub(mounts: MountEntry[]) {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = new URL(input.toString(), "http://localhost");
    if (url.pathname === "/api/mounts") {
      return new Response(JSON.stringify(mounts), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    return new Response("[]", { status: 200 });
  });
}

function renderBadge(hint = HINT) {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <UnresolvedBadge hint={hint} />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("UnresolvedBadge", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders the 'mount <slug>' copy with the hint project", () => {
    installMountsStub([]);
    renderBadge();
    expect(screen.getByText(/unresolved/i)).toBeInTheDocument();
    expect(screen.getByText("other-repo")).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: /init this mount/i }),
    ).not.toBeInTheDocument();
  });

  it("offers an 'init this mount' shortcut when a NeedsInit mount matches the slug", async () => {
    installMountsStub([
      {
        path: "/repos/other-repo",
        dirName: "other-repo",
        state: "needsInit",
      },
    ]);
    renderBadge();
    await waitFor(() =>
      expect(
        screen.getByRole("link", { name: /init this mount/i }),
      ).toBeInTheDocument(),
    );
  });

  it("does not offer the shortcut when the matching mount is already a Project", async () => {
    installMountsStub([
      {
        path: "/repos/other-repo",
        dirName: "other-repo",
        state: "project",
        project: {
          slug: "other-repo",
          name: "Other",
          description: null,
          collectionCount: 0,
          artifactCount: 0,
        },
      },
    ]);
    renderBadge();
    // Wait a tick so the mounts query has resolved.
    await waitFor(() =>
      expect(screen.getByText("other-repo")).toBeInTheDocument(),
    );
    expect(
      screen.queryByRole("link", { name: /init this mount/i }),
    ).not.toBeInTheDocument();
  });
});
