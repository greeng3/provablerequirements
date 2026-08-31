import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { InitProjectDialog } from "../InitProjectDialog";

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
        <InitProjectDialog dirName="sample" onClose={onClose} />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

const INIT_RESPONSE = {
  slug: "sample",
  name: "Sample",
  description: null,
  artifactsPath: "artifacts",
  collections: [],
};

const SAMPLE_RESPONSE = {
  projectSlug: "sample",
  collectionsCreated: 3,
  artifactsCreated: 7,
  collections: [
    {
      prefix: "REQ",
      directoryName: "requirements",
      artifactCount: 3,
      artifactNames: [],
    },
    {
      prefix: "DES",
      directoryName: "design",
      artifactCount: 2,
      artifactNames: [],
    },
    {
      prefix: "UC",
      directoryName: "use-cases",
      artifactCount: 2,
      artifactNames: [],
    },
  ],
};

describe("InitProjectDialog post-init choice", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("advances from form to the three-button choice after init succeeds", async () => {
    stubFetch((url, method) => {
      if (url.includes("/api/mounts/") && method === "POST") {
        return { status: 201, body: INIT_RESPONSE };
      }
      return { status: 404, body: {} };
    });
    mount();
    await userEvent.click(screen.getByRole("button", { name: /Initialise$/ }));
    await waitFor(() =>
      expect(screen.getByTestId("init-post-choice")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("init-choice-empty")).toBeInTheDocument();
    expect(screen.getByTestId("init-choice-sample")).toBeInTheDocument();
    expect(screen.getByTestId("init-choice-doorstop")).toBeInTheDocument();
  });

  it('"Create sample content" posts to the seed endpoint and closes on success', async () => {
    const calls = stubFetch((url, method) => {
      if (url.includes("/api/mounts/") && method === "POST") {
        return { status: 201, body: INIT_RESPONSE };
      }
      if (url.endsWith("/sample-content") && method === "POST") {
        return { status: 201, body: SAMPLE_RESPONSE };
      }
      return { status: 404, body: {} };
    });
    const onClose = vi.fn();
    mount(onClose);
    await userEvent.click(screen.getByRole("button", { name: /Initialise$/ }));
    await userEvent.click(await screen.findByTestId("init-choice-sample"));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(
      calls.some(
        (c) =>
          c.method === "POST" &&
          c.url.endsWith("/api/projects/sample/sample-content"),
      ),
    ).toBe(true);
  });

  it('"Create sample content" surfaces the 409 error instead of closing', async () => {
    stubFetch((url, method) => {
      if (url.includes("/api/mounts/") && method === "POST") {
        return { status: 201, body: INIT_RESPONSE };
      }
      if (url.endsWith("/sample-content") && method === "POST") {
        return { status: 409, body: { error: "already has 1 collection" } };
      }
      return { status: 404, body: {} };
    });
    const onClose = vi.fn();
    mount(onClose);
    await userEvent.click(screen.getByRole("button", { name: /Initialise$/ }));
    await userEvent.click(await screen.findByTestId("init-choice-sample"));
    await waitFor(() =>
      expect(screen.getByTestId("init-choice-error")).toBeInTheDocument(),
    );
    expect(onClose).not.toHaveBeenCalled();
  });

  it('"Start empty" closes without a seed request', async () => {
    const calls = stubFetch((url, method) => {
      if (url.includes("/api/mounts/") && method === "POST") {
        return { status: 201, body: INIT_RESPONSE };
      }
      return { status: 404, body: {} };
    });
    const onClose = vi.fn();
    mount(onClose);
    await userEvent.click(screen.getByRole("button", { name: /Initialise$/ }));
    await userEvent.click(await screen.findByTestId("init-choice-empty"));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(calls.some((c) => c.url.endsWith("/sample-content"))).toBe(false);
  });
});
