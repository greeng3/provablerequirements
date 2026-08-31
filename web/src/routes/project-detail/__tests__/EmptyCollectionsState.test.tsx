import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { EmptyCollectionsState } from "../EmptyCollectionsState";

interface HandlerResult {
  status: number;
  body: unknown;
}

function stubFetch(handler: (url: string, method: string) => HandlerResult) {
  const calls: Array<{ url: string; method: string }> = [];
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";
    calls.push({ url, method });
    const result = handler(url, method);
    return new Response(JSON.stringify(result.body), {
      status: result.status,
      headers: { "content-type": "application/json" },
    });
  });
  return calls;
}

function mount() {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <EmptyCollectionsState projectSlug="sample" />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("EmptyCollectionsState", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders guidance plus a Create-sample-content button", () => {
    stubFetch(() => ({ status: 404, body: {} }));
    mount();
    expect(screen.getByText(/No Collections yet/i)).toBeInTheDocument();
    expect(
      screen.getByTestId("empty-state-sample-content"),
    ).toBeInTheDocument();
  });

  it("posts to the seed endpoint when the button is clicked", async () => {
    const calls = stubFetch((url, method) => {
      if (url.endsWith("/sample-content") && method === "POST") {
        return {
          status: 201,
          body: {
            projectSlug: "sample",
            collectionsCreated: 3,
            artifactsCreated: 7,
            collections: [],
          },
        };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(screen.getByTestId("empty-state-sample-content"));
    await waitFor(() =>
      expect(
        calls.some(
          (c) =>
            c.method === "POST" &&
            c.url.endsWith("/api/projects/sample/sample-content"),
        ),
      ).toBe(true),
    );
  });

  it("surfaces the 409 body when the project is no longer empty", async () => {
    stubFetch((url, method) => {
      if (url.endsWith("/sample-content") && method === "POST") {
        return {
          status: 409,
          body: { error: "project 'sample' already has 1 collection" },
        };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(screen.getByTestId("empty-state-sample-content"));
    await waitFor(() =>
      expect(screen.getByTestId("empty-state-sample-error")).toHaveTextContent(
        /already has/i,
      ),
    );
  });
});
