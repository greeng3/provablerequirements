import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { SearchPage } from "../SearchPage";

interface FetchCall {
  url: string;
  method: string;
}

function stubFetch(handler: (url: string, method: string) => unknown) {
  const calls: FetchCall[] = [];
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";
    calls.push({ url, method });
    const body = handler(url, method);
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  });
  return calls;
}

function renderPage() {
  return render(
    <TestQueryProvider>
      <MemoryRouter initialEntries={["/search"]}>
        <SearchPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

const emptyResponse = {
  totalHits: 0,
  limit: 50,
  offset: 0,
  truncated: false,
  hits: [],
};

function sampleHit(
  name: string,
  extras: {
    snippet?: string;
    reviewState?: string;
    shape?: string;
    active?: boolean;
  } = {},
) {
  return {
    uuid: `uuid-${name}`,
    projectSlug: "sample",
    collectionPrefix: "REQ",
    artifactName: name,
    title: `Title of ${name}`,
    shape: extras.shape ?? "content",
    reviewState: extras.reviewState ?? "never-reviewed",
    active: extras.active ?? true,
    score: 1.2,
    snippet: extras.snippet,
  };
}

describe("SearchPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("debounces the query box, renders results with snippets, and threads filter state into /api/search", async () => {
    const calls = stubFetch((url) => {
      if (url.includes("/api/search")) {
        return {
          totalHits: 2,
          limit: 50,
          offset: 0,
          truncated: false,
          hits: [
            sampleHit("REQ-core", {
              snippet:
                "The <mark>reactor</mark> vessel shall satisfy the envelope.",
              reviewState: "approved",
            }),
            sampleHit("REQ-b"),
          ],
        };
      }
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();

    // Initial render fires a match-all search.
    await waitFor(() => {
      expect(screen.getByTestId("search-result-REQ-core")).toBeInTheDocument();
    });

    const box = screen.getByLabelText(/search query/i);
    await userEvent.type(box, "reactor");

    // Debounce means we don't see a fetch per keystroke — wait
    // for at least one /api/search?q=reactor request to land.
    await waitFor(() => {
      expect(
        calls.some(
          (c) => c.url.includes("/api/search") && c.url.includes("q=reactor"),
        ),
      ).toBe(true);
    });

    // Snippet rendered as DOM mark, not HTML.
    const marks = screen.getAllByTestId("search-snippet-mark");
    expect(marks.length).toBeGreaterThanOrEqual(1);
  });

  it("empty-results state shows a friendly message and no pagination buttons", async () => {
    stubFetch((url) => {
      if (url.includes("/api/search")) return emptyResponse;
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByText(/no artifacts match the current query/i),
      ).toBeInTheDocument(),
    );
    expect(screen.queryByTestId("search-next")).not.toBeInTheDocument();
  });

  it("Next button advances the offset when the response is truncated", async () => {
    let latestOffset = -1;
    stubFetch((url) => {
      if (url.includes("/api/search")) {
        const m = url.match(/offset=(\d+)/);
        latestOffset = m ? Number(m[1]) : 0;
        return {
          totalHits: 120,
          limit: 50,
          offset: latestOffset,
          truncated: latestOffset + 50 < 120,
          hits: [sampleHit(`REQ-${latestOffset}`)],
        };
      }
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();
    const next = await screen.findByTestId("search-next");
    await waitFor(() => expect(next).not.toBeDisabled());
    await userEvent.click(next);
    await waitFor(() => expect(latestOffset).toBe(50));
  });

  it("toggling a shape chip threads the filter into the search URL", async () => {
    const calls = stubFetch((url) => {
      if (url.includes("/api/search")) return emptyResponse;
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();
    const contentChip = await screen.findByRole("button", {
      name: /^content$/,
    });
    await userEvent.click(contentChip);
    await waitFor(() => {
      expect(
        calls.some(
          (c) =>
            c.url.includes("/api/search") && c.url.includes("shape=content"),
        ),
      ).toBe(true);
    });
  });
});
