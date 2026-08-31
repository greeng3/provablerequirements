import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import { useMounts, useProjects } from "../queries";

function stubFetch(handler: (url: string) => Response) {
  vi.stubGlobal("fetch", async (input: RequestInfo) => {
    const url = typeof input === "string" ? input : input.toString();
    return handler(url);
  });
}

function wrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

function json(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

describe("react-query hooks", () => {
  it("useProjects resolves to the project list", async () => {
    stubFetch(() =>
      json([
        {
          slug: "a",
          name: "A",
          description: null,
          collectionCount: 1,
          artifactCount: 2,
        },
      ]),
    );
    const { result } = renderHook(() => useProjects(), {
      wrapper: wrapper(),
    });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual([
      {
        slug: "a",
        name: "A",
        description: null,
        collectionCount: 1,
        artifactCount: 2,
      },
    ]);
  });

  it("useMounts resolves to the mount list", async () => {
    stubFetch(() =>
      json([
        { path: "/repos/a", dirName: "a", state: "needsInit" },
        { path: "/repos/b", dirName: "b", state: "noGit" },
      ]),
    );
    const { result } = renderHook(() => useMounts(), { wrapper: wrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toHaveLength(2);
    expect(result.current.data?.[0].state).toBe("needsInit");
  });

  it("useProjects surfaces an error when fetch fails", async () => {
    vi.stubGlobal(
      "fetch",
      async () =>
        new Response("{}", {
          status: 500,
          headers: { "content-type": "application/json" },
        }),
    );
    const { result } = renderHook(() => useProjects(), {
      wrapper: wrapper(),
    });
    await waitFor(() => expect(result.current.isError).toBe(true));
  });
});
