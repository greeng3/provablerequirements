import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { DoorstopRenameWizard } from "../DoorstopRenameWizard";

interface FetchCall {
  url: string;
  method: string;
  body?: string;
}

interface HandlerResult {
  status: number;
  body: unknown;
}

function stubFetch(
  handler: (url: string, method: string, body?: string) => HandlerResult,
) {
  const calls: FetchCall[] = [];
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

const REQ_ARTIFACTS = [
  {
    uuid: "11111111-1111-1111-1111-111111111111",
    name: "REQ-one",
    title: "First",
    shape: "content",
    active: true,
    reviewState: "never-reviewed",
  },
  {
    uuid: "22222222-2222-2222-2222-222222222222",
    name: "REQ-two",
    title: "Second",
    shape: "content",
    active: true,
    reviewState: "never-reviewed",
  },
];

const BULK_OK_RESPONSE = {
  results: [
    {
      kind: "ok",
      uuid: "11111111-1111-1111-1111-111111111111",
      suggestions: [
        { name: "REQ-one-better", rationale: "clearer phrasing" },
        { name: "REQ-alpha", rationale: "shorter" },
      ],
      servedByIndex: 0,
      servedBy: "anthropic/claude-haiku-4-5",
    },
    {
      kind: "ok",
      uuid: "22222222-2222-2222-2222-222222222222",
      suggestions: [{ name: "REQ-two-better", rationale: "clearer phrasing" }],
      servedByIndex: 0,
      servedBy: "anthropic/claude-haiku-4-5",
    },
  ],
};

function mount(onClose = () => {}) {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <DoorstopRenameWizard
          projectSlug="sample"
          collectionPrefixes={["REQ"]}
          onClose={onClose}
        />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("DoorstopRenameWizard", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("fetches artifacts then bulk suggestions and renders a row per artifact", async () => {
    stubFetch((url, method) => {
      if (
        url.includes("/api/projects/sample/collections/REQ/artifacts") &&
        method === "GET"
      ) {
        return { status: 200, body: REQ_ARTIFACTS };
      }
      if (
        url.endsWith("/api/projects/sample/rename-suggestions/bulk") &&
        method === "POST"
      ) {
        return { status: 200, body: BULK_OK_RESPONSE };
      }
      return { status: 404, body: {} };
    });
    mount();
    await waitFor(() =>
      expect(
        screen.getByTestId("doorstop-rename-wizard-list"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText("First")).toBeInTheDocument();
    expect(screen.getByText("Second")).toBeInTheDocument();
    expect(
      screen.getByTestId("doorstop-rename-wizard-suggest-REQ-one-better"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("doorstop-rename-wizard-suggest-REQ-two-better"),
    ).toBeInTheDocument();
  });

  it("picks suggestions and applies them via PATCH rename, one per picked row", async () => {
    const patchCalls: FetchCall[] = [];
    stubFetch((url, method, body) => {
      if (
        url.includes("/api/projects/sample/collections/REQ/artifacts") &&
        method === "GET"
      ) {
        return { status: 200, body: REQ_ARTIFACTS };
      }
      if (
        url.endsWith("/api/projects/sample/rename-suggestions/bulk") &&
        method === "POST"
      ) {
        return { status: 200, body: BULK_OK_RESPONSE };
      }
      if (url.includes("/api/artifacts/") && method === "PATCH") {
        patchCalls.push({ url, method, body });
        return { status: 200, body: {} };
      }
      return { status: 404, body: {} };
    });
    mount();
    await waitFor(() =>
      expect(
        screen.getByTestId("doorstop-rename-wizard-list"),
      ).toBeInTheDocument(),
    );

    // Pick one suggestion on each row.
    await userEvent.click(
      screen.getByTestId("doorstop-rename-wizard-suggest-REQ-one-better"),
    );
    await userEvent.click(
      screen.getByTestId("doorstop-rename-wizard-suggest-REQ-two-better"),
    );

    const apply = screen.getByTestId("doorstop-rename-wizard-apply");
    expect(apply).toHaveTextContent("Apply 2 renames");
    await userEvent.click(apply);

    await waitFor(() => expect(patchCalls.length).toBe(2));
    const bodies = patchCalls.map((c) => JSON.parse(c.body ?? "{}"));
    expect(bodies).toContainEqual({ name: "REQ-one-better" });
    expect(bodies).toContainEqual({ name: "REQ-two-better" });
  });

  it("renders the privacy-ack arm with a link to /llm for rows the backend flagged", async () => {
    stubFetch((url, method) => {
      if (
        url.includes("/api/projects/sample/collections/REQ/artifacts") &&
        method === "GET"
      ) {
        return { status: 200, body: [REQ_ARTIFACTS[0]] };
      }
      if (
        url.endsWith("/api/projects/sample/rename-suggestions/bulk") &&
        method === "POST"
      ) {
        return {
          status: 200,
          body: {
            results: [
              {
                kind: "privacyAckRequired",
                uuid: "11111111-1111-1111-1111-111111111111",
                indices: [0],
              },
            ],
          },
        };
      }
      return { status: 404, body: {} };
    });
    mount();
    await waitFor(() =>
      expect(
        screen.getByTestId("doorstop-rename-wizard-list"),
      ).toBeInTheDocument(),
    );
    const link = screen.getByRole("link", { name: /LLM providers/i });
    expect(link).toHaveAttribute("href", "/llm");
  });
});
