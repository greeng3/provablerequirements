import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ReviewQueuePage } from "../ReviewQueuePage";
import { TestQueryProvider } from "../../test-utils";

const QUEUE = {
  awaitingReview: [
    {
      uuid: "0194f6d0-0001-7000-8000-000000000001",
      projectSlug: "sample",
      collectionPrefix: "REQ",
      artifactName: "REQ-awaiting",
      title: "Awaiting",
      shape: "content",
      state: "neverReviewed",
      lastEventAt: null,
      modifiedAt: "2026-04-18T00:00:00Z",
      blockingTodoCount: 0,
      tags: [],
      lastReviewer: null,
    },
  ],
  blockingTodos: [
    {
      uuid: "0194f6d0-0001-7000-8000-000000000002",
      projectSlug: "sample",
      collectionPrefix: "REQ",
      artifactName: "REQ-blocked",
      title: "Blocked",
      shape: "content",
      state: "rejected",
      lastEventAt: "2026-04-20T00:00:00Z",
      modifiedAt: "2026-04-18T00:00:00Z",
      blockingTodoCount: 2,
      tags: [],
      lastReviewer: "bob",
    },
  ],
};

function installFetchStub(queue = QUEUE) {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = new URL(input.toString(), "http://localhost");
    if (url.pathname === "/api/reviews/queue") {
      return new Response(JSON.stringify(queue), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    return new Response("{}", { status: 200 });
  });
}

function renderPage() {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <ReviewQueuePage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("ReviewQueuePage", () => {
  beforeEach(() => installFetchStub());
  afterEach(() => vi.restoreAllMocks());

  it("renders both queue sections with their entries", async () => {
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/REQ-awaiting/)).toBeInTheDocument(),
    );
    expect(screen.getByText(/REQ-blocked/)).toBeInTheDocument();
    expect(screen.getByText(/Awaiting review \(1\)/)).toBeInTheDocument();
    expect(screen.getByText(/Blocking TODOs \(1\)/)).toBeInTheDocument();
    // Blocking TODO count badge.
    expect(screen.getByText(/2 TODOs/)).toBeInTheDocument();
  });

  it("shows empty-section copy when a section is empty", async () => {
    installFetchStub({
      awaitingReview: [],
      blockingTodos: [],
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByText(/No artifacts awaiting review/),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText(/No blocking TODOs/)).toBeInTheDocument();
  });
});
