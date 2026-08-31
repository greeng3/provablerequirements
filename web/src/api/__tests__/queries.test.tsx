import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import {
  queryKeys,
  useArtifact,
  useArtifactDiff,
  useArtifactHistory,
  useArtifacts,
  useCollection,
  useCollections,
  useIncomingLinks,
  useProject,
  useProjects,
} from "../queries";

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

  // #385: a disabled hook must NOT register under another active
  // hook's real key. Sharing `queryKeys.projects` (useProjects' key)
  // means an invalidateQueries on the project list refetches the
  // disabled query with the WRONG queryFn, clobbering the cache.
  it("disabled hooks never share the active projects-list key", () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const w = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    // Every one of these is disabled (missing args) and used to fall
    // back to queryKeys.projects.
    renderHook(
      () => {
        useProject(undefined);
        useCollections(undefined);
        useCollection(undefined, undefined);
        useArtifacts(undefined, undefined);
        useArtifact(undefined);
        useIncomingLinks(undefined);
        useArtifactHistory(undefined);
        useArtifactDiff(undefined, undefined, undefined);
      },
      { wrapper: w },
    );
    const projectsKey = JSON.stringify(queryKeys.projects);
    const collisions = client
      .getQueryCache()
      .getAll()
      .filter((q) => JSON.stringify(q.queryKey) === projectsKey);
    expect(collisions).toHaveLength(0);
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
