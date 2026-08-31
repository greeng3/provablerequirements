import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ReviewPane } from "../ReviewPane";
import { TestQueryProvider } from "../../../test-utils";
import type { ArtifactDetail } from "../../../api/types";

function detail(overrides: Partial<ArtifactDetail> = {}): ArtifactDetail {
  return {
    name: "REQ-a",
    projectSlug: "sample",
    collectionPrefix: "REQ",
    uuid: "0194f6d0-0001-7000-8000-000000000001",
    title: "Sample",
    shape: "content",
    description: null,
    active: true,
    derived: false,
    createdAt: "2026-04-18T00:00:00Z",
    modifiedAt: "2026-04-18T00:00:00Z",
    tags: [],
    outlineLevel: undefined,
    links: [],
    reviewLog: [],
    reviewState: {
      state: "neverReviewed",
      blockingTodos: [],
    },
    body: "",
    ...overrides,
  };
}

function installFetchStub() {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = new URL(input.toString(), "http://localhost");
    let body: unknown = {};
    if (url.pathname === "/api/reviewers") {
      body = { gitDefault: "Alice", persisted: [], session: [] };
    } else if (url.pathname.endsWith("/last-approval-snapshot")) {
      body = {
        approvedAt: "2026-04-18T00:00:00Z",
        body: "# Approved\n",
        metadata: { title: "Sample", tags: [] },
      };
    }
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  });
}

function renderPane(artifact: ArtifactDetail) {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <ReviewPane artifact={artifact} />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("ReviewPane", () => {
  beforeEach(() => installFetchStub());
  afterEach(() => vi.restoreAllMocks());

  it("shows the never-reviewed banner on a fresh artifact", () => {
    renderPane(detail());
    expect(screen.getByText(/No prior approval/i)).toBeInTheDocument();
    expect(
      screen.getByLabelText(/review state: never reviewed/i),
    ).toBeInTheDocument();
  });

  it("disables Approve when blocking TODOs are open", () => {
    renderPane(
      detail({
        reviewState: {
          state: "rejected",
          lastEventAt: "2026-04-19T00:00:00Z",
          lastReviewer: "bob",
          blockingTodos: [
            {
              id: "t1",
              text: "Fix AC",
              addedAt: "2026-04-19T00:00:00Z",
              addedBy: "bob",
            },
          ],
        },
      }),
    );
    const approveButton = screen.getByRole("button", { name: /^approve$/i });
    expect(approveButton).toBeDisabled();
    expect(screen.getByText("Fix AC")).toBeInTheDocument();
  });

  it("opens the approve dialog when the button is clicked", async () => {
    const user = userEvent.setup();
    renderPane(detail());
    await user.click(screen.getByRole("button", { name: /^approve$/i }));
    expect(
      screen.getByRole("dialog", { name: /approve review/i }),
    ).toBeInTheDocument();
    // Cancel closes it again.
    await user.click(screen.getByRole("button", { name: /cancel/i }));
    expect(
      screen.queryByRole("dialog", { name: /approve review/i }),
    ).not.toBeInTheDocument();
  });

  it("re-request-review is disabled when there is no prior history", () => {
    renderPane(detail());
    const button = screen.getByRole("button", { name: /re-request/i });
    expect(button).toBeDisabled();
  });

  it("renders the since-last-approval section only when there has been an approval", async () => {
    renderPane(
      detail({
        reviewState: {
          state: "approved",
          lastApprovalAt: "2026-04-18T00:00:00Z",
          lastEventAt: "2026-04-18T00:00:00Z",
          lastReviewer: "alice",
          blockingTodos: [],
        },
        reviewLog: [
          {
            timestamp: "2026-04-18T00:00:00Z",
            reviewer: "alice",
            outcome: "approved",
          },
        ],
      }),
    );
    await waitFor(() =>
      expect(screen.getByText(/Since last approval/)).toBeInTheDocument(),
    );
  });
});
