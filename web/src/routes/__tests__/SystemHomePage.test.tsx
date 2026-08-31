import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../test-utils";
import { SystemHomePage } from "../SystemHomePage";

function stubMounts(body: unknown, status = 200) {
  vi.stubGlobal(
    "fetch",
    async () =>
      new Response(JSON.stringify(body), {
        status,
        headers: { "content-type": "application/json" },
      }),
  );
}

function renderPage() {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <SystemHomePage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SystemHomePage", () => {
  it("renders a project mount with link, counts, and Project badge", async () => {
    stubMounts([
      {
        path: "/repos/sample-project",
        dirName: "sample-project",
        state: "project",
        project: {
          slug: "sample-project",
          name: "Sample Project",
          description: "Demo fixture",
          collectionCount: 2,
          artifactCount: 5,
        },
      },
    ]);

    renderPage();

    await waitFor(() =>
      expect(screen.getByText(/Sample Project/)).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("link", { name: /sample-project/i }),
    ).toHaveAttribute("href", "/projects/sample-project");
    expect(screen.getByLabelText(/mount state: Project/i)).toBeInTheDocument();
    expect(screen.getByText(/collections?$/)).toBeInTheDocument();
    expect(screen.getByText(/artifacts?$/)).toBeInTheDocument();
    // Counts live in their own <span> inside the totals block.
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("5")).toBeInTheDocument();
  });

  it("renders needs-init mounts with the right badge and no link", async () => {
    stubMounts([
      {
        path: "/repos/needs-init",
        dirName: "needs-init",
        state: "needsInit",
      },
    ]);

    renderPage();

    await waitFor(() =>
      expect(
        screen.getByLabelText(/mount state: Needs init/i),
      ).toBeInTheDocument(),
    );
    expect(screen.queryByRole("link", { name: /needs-init/i })).toBeNull();
    expect(screen.getByText(/no reqforge\.json/i)).toBeInTheDocument();
  });

  it("renders no-git mounts with the right badge and guidance", async () => {
    stubMounts([{ path: "/repos/no-git", dirName: "no-git", state: "noGit" }]);

    renderPage();

    await waitFor(() =>
      expect(screen.getByLabelText(/mount state: No git/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/git init/)).toBeInTheDocument();
  });

  it("renders load-failed mounts with the error message", async () => {
    stubMounts([
      {
        path: "/repos/broken",
        dirName: "broken",
        state: "loadFailed",
        error: "invalid reqforge.json: missing slug",
      },
    ]);

    renderPage();

    await waitFor(() =>
      expect(
        screen.getByLabelText(/mount state: Load failed/i),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/invalid reqforge\.json: missing slug/i),
    ).toBeInTheDocument();
  });

  it("shows explicit bind-mount guidance when no mounts are found", async () => {
    stubMounts([]);

    renderPage();

    await waitFor(() =>
      expect(screen.getByText(/No repositories mounted/i)).toBeInTheDocument(),
    );
    // UX-startupHomeView: the empty state must include concrete
    // next-step guidance, not a bare "no items" message.
    expect(screen.getByText(/docker-compose\.yml/i)).toBeInTheDocument();
    // "docker run" appears twice (the section heading and the
    // code snippet). getAllByText picks both up.
    expect(screen.getAllByText(/docker run/).length).toBeGreaterThan(0);
  });

  it("shows an error when the backend is unreachable", async () => {
    vi.stubGlobal("fetch", async () => {
      throw new Error("network down");
    });

    renderPage();

    await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
  });
});
