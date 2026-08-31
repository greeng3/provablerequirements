import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { RenameArtifactDialog } from "../RenameArtifactDialog";
import type { ArtifactDetail } from "../../../api/types";

interface HandlerResult {
  status: number;
  body: unknown;
}

function stubFetch(handler: (url: string, method: string) => HandlerResult) {
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";
    const result = handler(url, method);
    return new Response(JSON.stringify(result.body), {
      status: result.status,
      headers: { "content-type": "application/json" },
    });
  });
}

const artifact: ArtifactDetail = {
  uuid: "11111111-1111-1111-1111-111111111111",
  name: "REQ-old-name",
  title: "Pressure envelope",
  shape: "content",
  description: null,
  active: true,
  derived: false,
  createdAt: "2026-04-23T00:00:00Z",
  modifiedAt: "2026-04-23T00:00:00Z",
  projectSlug: "sample",
  collectionPrefix: "REQ",
  body: "body",
  links: [],
  tags: [],
  reviewLog: [],
  reviewState: { state: "neverReviewed", blockingTodos: [] },
};

function mount(onClose = () => {}) {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <RenameArtifactDialog artifact={artifact} onClose={onClose} />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("RenameArtifactDialog", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("hides the Suggest panel when no LLM providers are configured", async () => {
    stubFetch((url) => {
      if (url.endsWith("/api/llm/providers")) {
        return { status: 200, body: { providers: [] } };
      }
      return { status: 404, body: {} };
    });
    mount();
    // Wait for the /api/llm/providers query to settle (otherwise
    // we might race the suspense fallback off).
    await waitFor(() => {
      expect(screen.queryByTestId("rename-suggest-panel")).toBeNull();
    });
  });

  it("shows the Suggest panel and lists suggestions after clicking", async () => {
    let suggestionsCallCount = 0;
    stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers")) {
        return {
          status: 200,
          body: {
            providers: [
              {
                index: 0,
                provider: "anthropic",
                model: "claude-haiku-4-5",
                endpoint: "https://api.anthropic.com",
                isLocal: false,
                requiresPrivacyAck: false,
                apiKeyEnvVar: "ANTHROPIC_API_KEY",
                apiKeyAvailable: true,
                health: { kind: "healthy" },
              },
            ],
          },
        };
      }
      if (url.includes("/rename-suggestions") && method === "POST") {
        suggestionsCallCount += 1;
        return {
          status: 200,
          body: {
            kind: "ok",
            suggestions: [
              {
                name: "REQ-pressure-containment",
                rationale: "aligns with sibling stem",
              },
              { name: "REQ-vessel-pressure", rationale: "shorter phrasing" },
            ],
            servedByIndex: 0,
            servedBy: "anthropic/claude-haiku-4-5",
          },
        };
      }
      return { status: 404, body: {} };
    });
    mount();
    const suggestButton = await screen.findByTestId("rename-suggest-button");
    await userEvent.click(suggestButton);
    await waitFor(() =>
      expect(screen.getByTestId("rename-suggest-list")).toBeInTheDocument(),
    );
    expect(suggestionsCallCount).toBe(1);
    expect(
      screen.getByTestId("rename-suggest-pick-REQ-pressure-containment"),
    ).toBeInTheDocument();
  });

  it("picks a suggestion into the text field", async () => {
    stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers")) {
        return {
          status: 200,
          body: {
            providers: [
              {
                index: 0,
                provider: "anthropic",
                model: "claude-haiku-4-5",
                endpoint: "https://api.anthropic.com",
                isLocal: false,
                requiresPrivacyAck: false,
                apiKeyEnvVar: "KEY",
                apiKeyAvailable: true,
                health: { kind: "healthy" },
              },
            ],
          },
        };
      }
      if (url.includes("/rename-suggestions") && method === "POST") {
        return {
          status: 200,
          body: {
            kind: "ok",
            suggestions: [
              { name: "REQ-new-stem", rationale: "better phrasing" },
            ],
            servedByIndex: 0,
            servedBy: "anthropic/claude-haiku-4-5",
          },
        };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(await screen.findByTestId("rename-suggest-button"));
    const pick = await screen.findByTestId("rename-suggest-pick-REQ-new-stem");
    await userEvent.click(pick);
    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.value).toBe("REQ-new-stem");
  });

  it("surfaces the privacy-ack-required arm with a link to /llm", async () => {
    stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers")) {
        return {
          status: 200,
          body: {
            providers: [
              {
                index: 0,
                provider: "anthropic",
                model: "claude-haiku-4-5",
                endpoint: "https://api.anthropic.com",
                isLocal: false,
                requiresPrivacyAck: true,
                apiKeyEnvVar: "KEY",
                apiKeyAvailable: true,
                health: { kind: "healthy" },
              },
            ],
          },
        };
      }
      if (url.includes("/rename-suggestions") && method === "POST") {
        return {
          status: 200,
          body: { kind: "privacyAckRequired", indices: [0] },
        };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(await screen.findByTestId("rename-suggest-button"));
    await waitFor(() =>
      expect(
        screen.getByTestId("rename-suggest-privacy-alert"),
      ).toBeInTheDocument(),
    );
    const link = screen.getByRole("link", { name: /LLM providers/i });
    expect(link).toHaveAttribute("href", "/llm");
  });
});
