import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { SuggestedLinksTab } from "../SuggestedLinksTab";

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
    return new Response(
      result.body === undefined ? null : JSON.stringify(result.body),
      {
        status: result.status,
        headers: { "content-type": "application/json" },
      },
    );
  });
  return calls;
}

function mount() {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <SuggestedLinksTab projectSlug="reqforge" />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

const SAMPLE_PENDING = {
  suggestions: [
    {
      id: "0194f6d0-0001-7000-8000-000000000001",
      from: "0194f6d0-0001-7000-8000-0000000000aa",
      to: "0194f6d0-0001-7000-8000-0000000000bb",
      linkType: "derives-from",
      confidence: 0.85,
      rationale: "REQ-aa describes a constraint that ART-bb implements",
    },
  ],
};

const SAMPLE_DECLINED = {
  declined: [
    {
      id: "0194f6d0-0001-7000-8000-000000000099",
      from: "0194f6d0-0001-7000-8000-0000000000aa",
      to: "0194f6d0-0001-7000-8000-0000000000cc",
      linkType: "satisfies",
      confidence: 0.5,
      rationale: "weak overlap",
      declinedAt: "2026-05-04T12:00:00Z",
    },
  ],
};

const NO_PROVIDERS = { providers: [] };
const ONE_PROVIDER = {
  providers: [
    {
      index: 0,
      provider: "openaiCompatible",
      model: "gpt-4o-mini",
      endpoint: "http://test/v1",
      apiKeyAvailable: true,
      health: "healthy",
      privacyAcknowledged: true,
    },
  ],
};

describe("SuggestedLinksTab", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders empty pending state and disabled analyze button when no LLM is configured", async () => {
    stubFetch((url) => {
      if (url.includes("/api/llm/providers"))
        return { status: 200, body: NO_PROVIDERS };
      if (url.endsWith("/suggestions/links"))
        return { status: 200, body: { suggestions: [] } };
      if (url.endsWith("/suggestions/links/declined"))
        return { status: 200, body: { declined: [] } };
      return { status: 404, body: {} };
    });
    mount();
    await waitFor(() =>
      expect(
        screen.getByTestId("suggested-links-pending-empty"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByTestId("analyze-link-suggestions")).toBeDisabled();
  });

  it("lists pending suggestions and POSTs accept on click", async () => {
    const calls = stubFetch((url, method) => {
      if (url.includes("/api/llm/providers"))
        return { status: 200, body: ONE_PROVIDER };
      if (method === "GET" && url.endsWith("/suggestions/links/declined"))
        return { status: 200, body: { declined: [] } };
      if (method === "GET" && url.endsWith("/suggestions/links"))
        return { status: 200, body: SAMPLE_PENDING };
      if (method === "POST" && url.includes("/accept"))
        return { status: 204, body: undefined };
      return { status: 404, body: { error: "unexpected" } };
    });
    mount();
    await waitFor(() =>
      expect(
        screen.getByTestId("suggested-links-pending-list"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText(/derives-from/)).toBeInTheDocument();
    expect(screen.getByText(/85% confidence/)).toBeInTheDocument();

    await userEvent.click(screen.getByTestId("suggested-link-accept"));
    await waitFor(() => {
      const acceptCall = calls.find(
        (c) => c.method === "POST" && c.url.includes("/accept"),
      );
      expect(acceptCall).toBeDefined();
      expect(acceptCall!.url).toContain(
        "/api/projects/reqforge/suggestions/links/0194f6d0-0001-7000-8000-000000000001/accept",
      );
    });
  });

  it("POSTs reject on click from the pending tab", async () => {
    const calls = stubFetch((url, method) => {
      if (url.includes("/api/llm/providers"))
        return { status: 200, body: ONE_PROVIDER };
      if (method === "GET" && url.endsWith("/suggestions/links/declined"))
        return { status: 200, body: { declined: [] } };
      if (method === "GET" && url.endsWith("/suggestions/links"))
        return { status: 200, body: SAMPLE_PENDING };
      if (method === "POST" && url.includes("/reject"))
        return { status: 204, body: undefined };
      return { status: 404, body: { error: "unexpected" } };
    });
    mount();
    await waitFor(() =>
      expect(
        screen.getByTestId("suggested-links-pending-list"),
      ).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByTestId("suggested-link-reject"));
    await waitFor(() => {
      const rejectCall = calls.find(
        (c) => c.method === "POST" && c.url.includes("/reject"),
      );
      expect(rejectCall).toBeDefined();
    });
  });

  it("switches to rejected sub-tab and reinstates a declined suggestion", async () => {
    const calls = stubFetch((url, method) => {
      if (url.includes("/api/llm/providers"))
        return { status: 200, body: ONE_PROVIDER };
      if (method === "GET" && url.endsWith("/suggestions/links/declined"))
        return { status: 200, body: SAMPLE_DECLINED };
      if (method === "GET" && url.endsWith("/suggestions/links"))
        return { status: 200, body: { suggestions: [] } };
      if (method === "POST" && url.includes("/reinstate"))
        return { status: 204, body: undefined };
      return { status: 404, body: { error: "unexpected" } };
    });
    mount();
    await userEvent.click(screen.getByTestId("suggested-links-rejected-tab"));
    await waitFor(() =>
      expect(
        screen.getByTestId("suggested-links-rejected-list"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText(/satisfies/)).toBeInTheDocument();
    expect(screen.getByText(/rejected/)).toBeInTheDocument();
    await userEvent.click(screen.getByTestId("suggested-link-reinstate"));
    await waitFor(() => {
      const reinstateCall = calls.find(
        (c) => c.method === "POST" && c.url.includes("/reinstate"),
      );
      expect(reinstateCall).toBeDefined();
    });
  });

  it("shows the noProviders banner when analyze returns kind=noProviders", async () => {
    stubFetch((url, method) => {
      if (url.includes("/api/llm/providers"))
        return { status: 200, body: ONE_PROVIDER };
      if (method === "GET" && url.endsWith("/suggestions/links/declined"))
        return { status: 200, body: { declined: [] } };
      if (method === "GET" && url.endsWith("/suggestions/links"))
        return { status: 200, body: { suggestions: [] } };
      if (method === "POST" && url.includes("/analyze"))
        return { status: 200, body: { kind: "noProviders" } };
      return { status: 404, body: { error: "unexpected" } };
    });
    mount();
    await waitFor(() =>
      expect(screen.getByTestId("analyze-link-suggestions")).toBeEnabled(),
    );
    await userEvent.click(screen.getByTestId("analyze-link-suggestions"));
    await waitFor(() =>
      expect(
        screen.getByTestId("analyze-status-noproviders"),
      ).toBeInTheDocument(),
    );
  });

  it("shows the ok banner with served-by attribution after a successful analyze", async () => {
    stubFetch((url, method) => {
      if (url.includes("/api/llm/providers"))
        return { status: 200, body: ONE_PROVIDER };
      if (method === "GET" && url.endsWith("/suggestions/links/declined"))
        return { status: 200, body: { declined: [] } };
      if (method === "GET" && url.endsWith("/suggestions/links"))
        return { status: 200, body: { suggestions: [] } };
      if (method === "POST" && url.includes("/analyze"))
        return {
          status: 200,
          body: {
            kind: "ok",
            suggestions: SAMPLE_PENDING.suggestions,
            servedByIndex: 0,
            servedBy: "openaiCompatible/gpt-4o-mini",
          },
        };
      return { status: 404, body: { error: "unexpected" } };
    });
    mount();
    await waitFor(() =>
      expect(screen.getByTestId("analyze-link-suggestions")).toBeEnabled(),
    );
    await userEvent.click(screen.getByTestId("analyze-link-suggestions"));
    await waitFor(() =>
      expect(screen.getByTestId("analyze-status-ok")).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/openaiCompatible\/gpt-4o-mini/),
    ).toBeInTheDocument();
  });
});
