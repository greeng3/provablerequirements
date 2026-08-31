import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { LlmProvidersPage } from "../LlmProvidersPage";

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
        <LlmProvidersPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

const remoteHealthy = {
  index: 0,
  provider: "anthropic",
  model: "claude-haiku-4-5",
  endpoint: "https://api.anthropic.com",
  isLocal: false,
  requiresPrivacyAck: true,
  apiKeyAvailable: true,
  enabled: true,
  health: { kind: "healthy" },
};

const localKeyless = {
  index: 1,
  provider: "openai-compatible",
  model: "local-llama",
  endpoint: "http://127.0.0.1:11434",
  isLocal: true,
  requiresPrivacyAck: false,
  apiKeyAvailable: true,
  enabled: false,
  health: { kind: "hard-disabled" },
};

describe("LlmProvidersPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders empty-state copy when no providers are configured", async () => {
    stubFetch(() => ({ status: 200, body: { providers: [] } }));
    mount();
    await waitFor(() =>
      expect(screen.getByTestId("llm-providers-empty")).toBeInTheDocument(),
    );
  });

  it("lists each provider with health, key, and enabled state", async () => {
    stubFetch(() => ({
      status: 200,
      body: { providers: [remoteHealthy, localKeyless] },
    }));
    mount();
    await waitFor(() =>
      expect(screen.getByTestId("llm-provider-0")).toBeInTheDocument(),
    );
    const first = screen.getByTestId("llm-provider-0");
    expect(first).toHaveTextContent("anthropic");
    expect(first).toHaveTextContent("claude-haiku-4-5");
    expect(first).toHaveTextContent("healthy");
    expect(first).toHaveTextContent("warning not yet acknowledged");

    const second = screen.getByTestId("llm-provider-1");
    expect(second).toHaveTextContent("hard-disabled");
    expect(second).toHaveTextContent("disabled");
    expect(second).toHaveTextContent("local endpoint, no warning needed");
    expect(screen.queryByTestId("llm-ack-1")).toBeNull();
  });

  it("fires POST retest when Retest button is clicked", async () => {
    const calls = stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers") && method === "GET") {
        return { status: 200, body: { providers: [remoteHealthy] } };
      }
      if (url.includes("/retest") && method === "POST") {
        return {
          status: 200,
          body: { ok: true, health: { kind: "healthy" } },
        };
      }
      return { status: 404, body: {} };
    });
    mount();
    const retestButton = await screen.findByTestId("llm-retest-0");
    await userEvent.click(retestButton);
    await waitFor(() =>
      expect(
        calls.some(
          (c) =>
            c.method === "POST" &&
            c.url.endsWith("/api/llm/providers/0/retest"),
        ),
      ).toBe(true),
    );
  });

  it("submits Add provider form and POSTs the new entry", async () => {
    const calls = stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers") && method === "GET") {
        return { status: 200, body: { providers: [] } };
      }
      if (url.endsWith("/api/llm/providers") && method === "POST") {
        return { status: 204, body: undefined };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(screen.getByTestId("llm-add-toggle"));
    // Default provider is openai-compatible — endpoint required.
    await userEvent.type(
      screen.getByTestId("llm-form-model"),
      "qwen2.5-coder:14b",
    );
    await userEvent.type(
      screen.getByTestId("llm-form-endpoint"),
      "http://host.docker.internal:11434",
    );
    await userEvent.click(screen.getByTestId("llm-form-submit"));
    await waitFor(() => {
      const post = calls.find(
        (c) => c.method === "POST" && c.url.endsWith("/api/llm/providers"),
      );
      expect(post).toBeDefined();
      const body = JSON.parse(post!.body!) as {
        provider: string;
        model: string;
        endpoint?: string;
      };
      expect(body.provider).toBe("openai-compatible");
      expect(body.model).toBe("qwen2.5-coder:14b");
      expect(body.endpoint).toBe("http://host.docker.internal:11434");
    });
  });

  it("requires endpoint for openai-compatible before posting", async () => {
    stubFetch((url) => {
      if (url.endsWith("/api/llm/providers")) {
        return { status: 200, body: { providers: [] } };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(screen.getByTestId("llm-add-toggle"));
    await userEvent.type(screen.getByTestId("llm-form-model"), "x");
    await userEvent.click(screen.getByTestId("llm-form-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("llm-form-error")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("llm-form-error")).toHaveTextContent(/endpoint/i);
  });

  it("PATCHes enabled when the checkbox is toggled", async () => {
    const calls = stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers") && method === "GET") {
        return { status: 200, body: { providers: [remoteHealthy] } };
      }
      if (url.endsWith("/api/llm/providers/0") && method === "PATCH") {
        return { status: 204, body: undefined };
      }
      return { status: 404, body: {} };
    });
    mount();
    const enabledBox = await screen.findByTestId("llm-enabled-0");
    await userEvent.click(enabledBox);
    await waitFor(() => {
      const patch = calls.find(
        (c) => c.method === "PATCH" && c.url.endsWith("/api/llm/providers/0"),
      );
      expect(patch).toBeDefined();
      const body = JSON.parse(patch!.body!) as { enabled: boolean };
      expect(body.enabled).toBe(false);
    });
  });

  it("opens the Edit form, prefills fields, and PUTs on save (key on file unchanged)", async () => {
    const calls = stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers") && method === "GET") {
        return { status: 200, body: { providers: [remoteHealthy] } };
      }
      if (url.endsWith("/api/llm/providers/0") && method === "PUT") {
        return { status: 204, body: undefined };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(await screen.findByTestId("llm-edit-0"));
    expect(screen.getByTestId("llm-edit-form")).toBeInTheDocument();
    // Existing model prefilled.
    const modelInput = screen.getByTestId("llm-form-model") as HTMLInputElement;
    expect(modelInput.value).toBe("claude-haiku-4-5");
    // Bump the model and submit. API key field is hidden by
    // default in edit mode (key on file unchanged).
    await userEvent.clear(modelInput);
    await userEvent.type(modelInput, "claude-sonnet-4-6");
    await userEvent.click(screen.getByTestId("llm-form-submit"));
    await waitFor(() => {
      const put = calls.find(
        (c) => c.method === "PUT" && c.url.endsWith("/api/llm/providers/0"),
      );
      expect(put).toBeDefined();
      const body = JSON.parse(put!.body!) as {
        provider: string;
        model: string;
        apiKey?: string;
      };
      expect(body.provider).toBe("anthropic");
      expect(body.model).toBe("claude-sonnet-4-6");
      // apiKey field omitted when "keep key on file" is the
      // default — server preserves the existing key.
      expect(body.apiKey).toBeUndefined();
    });
  });

  it("DELETEs after a confirm step", async () => {
    const calls = stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers") && method === "GET") {
        return { status: 200, body: { providers: [remoteHealthy] } };
      }
      if (url.endsWith("/api/llm/providers/0") && method === "DELETE") {
        return { status: 204, body: undefined };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(await screen.findByTestId("llm-delete-0"));
    await userEvent.click(screen.getByTestId("llm-delete-confirm-0"));
    await waitFor(() => {
      const del = calls.find(
        (c) => c.method === "DELETE" && c.url.endsWith("/api/llm/providers/0"),
      );
      expect(del).toBeDefined();
    });
  });
});
