import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { MigrateSchemaDialog } from "../MigrateSchemaDialog";

interface HandlerResult {
  status: number;
  body: unknown;
}

function stubFetch(
  handler: (url: string, method: string, body?: string) => HandlerResult,
) {
  const calls: Array<{ url: string; method: string; body?: string }> = [];
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";
    const rawBody = typeof init?.body === "string" ? init.body : undefined;
    calls.push({ url, method, body: rawBody });
    const result = handler(url, method, rawBody);
    return new Response(JSON.stringify(result.body), {
      status: result.status,
      headers: { "content-type": "application/json" },
    });
  });
  return calls;
}

function mount(onClose = () => {}) {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <MigrateSchemaDialog projectSlug="sample" onClose={onClose} />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

const HAPPY_RESULT = {
  projectSlug: "sample",
  result: {
    filesScanned: 3,
    filesRewritten: 0,
    filesUpToDate: 3,
    failures: [],
    rewritten: [],
  },
};

describe("MigrateSchemaDialog", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("posts force=false by default and shows the all-current success banner", async () => {
    const calls = stubFetch((_, method) => {
      if (method === "POST") {
        return { status: 200, body: HAPPY_RESULT };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(screen.getByTestId("migrate-schema-run"));
    await waitFor(() =>
      expect(screen.getByTestId("migrate-schema-result")).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/already at the current schema/i),
    ).toBeInTheDocument();
    const postCall = calls.find((c) => c.method === "POST");
    expect(postCall).toBeDefined();
    expect(JSON.parse(postCall!.body!)).toEqual({ force: false });
  });

  it("surfaces the dirty-worktree 409 arm and then runs with force=true", async () => {
    let callIndex = 0;
    const calls = stubFetch((_, method) => {
      if (method !== "POST") return { status: 404, body: {} };
      callIndex += 1;
      if (callIndex === 1) {
        return {
          status: 409,
          body: { error: "worktree has uncommitted changes" },
        };
      }
      return { status: 200, body: HAPPY_RESULT };
    });
    mount();
    await userEvent.click(screen.getByTestId("migrate-schema-run"));
    await waitFor(() =>
      expect(screen.getByTestId("migrate-schema-dirty")).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByTestId("migrate-schema-force"));
    await waitFor(() =>
      expect(screen.getByTestId("migrate-schema-result")).toBeInTheDocument(),
    );
    const bodies = calls
      .filter((c) => c.method === "POST")
      .map((c) => JSON.parse(c.body ?? "{}"));
    expect(bodies).toEqual([{ force: false }, { force: true }]);
  });

  it("renders per-file failures when the migration surfaces any", async () => {
    stubFetch((_, method) => {
      if (method === "POST") {
        return {
          status: 200,
          body: {
            projectSlug: "sample",
            result: {
              filesScanned: 2,
              filesRewritten: 0,
              filesUpToDate: 1,
              failures: [
                {
                  path: "/sample/artifacts/req/REQ-future.md",
                  fileType: "artifact",
                  error:
                    "schema: artifact file has schemaVersion 99, which is newer than 1",
                },
              ],
              rewritten: [],
            },
          },
        };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(screen.getByTestId("migrate-schema-run"));
    await waitFor(() =>
      expect(screen.getByTestId("migrate-schema-failures")).toBeInTheDocument(),
    );
    expect(screen.getByText(/REQ-future\.md/i)).toBeInTheDocument();
    expect(screen.getByText(/schemaVersion 99/i)).toBeInTheDocument();
  });
});
