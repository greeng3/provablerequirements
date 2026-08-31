import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, api } from "../client";

type FetchCall = { url: string; init?: RequestInit };

function stubFetch(
  handler: (call: FetchCall) => Response | Promise<Response>,
): FetchCall[] {
  const calls: FetchCall[] = [];
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const call: FetchCall = { url, init };
    calls.push(call);
    return handler(call);
  });
  return calls;
}

function json(body: unknown, init?: ResponseInit) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
    ...init,
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("api client", () => {
  it("parses health JSON", async () => {
    stubFetch(() => json({ status: "ok" }));
    const result = await api.health();
    expect(result).toEqual({ status: "ok" });
  });

  it("sends the Accept header so the backend negotiates JSON", async () => {
    const calls = stubFetch(() => json({ ready: true }));
    await api.readiness();
    expect(calls[0]?.init?.headers).toMatchObject({
      Accept: "application/json",
    });
  });

  it("hits the right paths with slug/prefix/uuid encoding", async () => {
    const calls = stubFetch((call) => {
      if (call.url.endsWith("/api/projects/weird%2Fslug")) {
        return json({
          slug: "weird/slug",
          name: "n",
          description: null,
          artifactsPath: "artifacts",
          collections: [],
        });
      }
      if (call.url.endsWith("/api/artifacts/abc-123")) {
        return json({
          name: "a",
          projectSlug: "p",
          collectionPrefix: "REQ",
          uuid: "abc-123",
          title: "t",
          shape: "content",
          description: null,
          active: true,
          derived: false,
          createdAt: "2026-04-18T00:00:00Z",
          modifiedAt: "2026-04-18T00:00:00Z",
          tags: [],
          links: [],
          body: "body",
        });
      }
      return new Response("not found", { status: 404 });
    });
    await api.project("weird/slug");
    await api.artifact("abc-123");
    expect(calls.map((c) => c.url)).toEqual([
      "/api/projects/weird%2Fslug",
      "/api/artifacts/abc-123",
    ]);
  });

  it("throws ApiError for non-2xx responses and includes body.error", async () => {
    stubFetch(() => json({ error: "boom" }, { status: 500 }));
    await expect(api.projects()).rejects.toMatchObject({
      name: "ApiError",
      status: 500,
    });
    stubFetch(() => json({ error: "missing" }, { status: 404 }));
    await expect(api.project("nope")).rejects.toSatisfy((err: ApiError) =>
      err.message.includes("missing"),
    );
  });

  it("throws ApiError when the error body is not JSON", async () => {
    stubFetch(() => new Response("plain text error", { status: 502 }));
    await expect(api.health()).rejects.toMatchObject({
      name: "ApiError",
      status: 502,
    });
  });

  it("returns the full typed array for /api/mounts", async () => {
    stubFetch(() =>
      json([
        {
          path: "/repos/a",
          dirName: "a",
          state: "project",
          project: {
            slug: "a",
            name: "A",
            description: null,
            collectionCount: 1,
            artifactCount: 2,
          },
        },
        { path: "/repos/b", dirName: "b", state: "needsInit" },
        { path: "/repos/c", dirName: "c", state: "noGit" },
      ]),
    );
    const mounts = await api.mounts();
    expect(mounts).toHaveLength(3);
    expect(mounts[0].state).toBe("project");
    expect(mounts[0].project?.artifactCount).toBe(2);
    expect(mounts[1].state).toBe("needsInit");
  });
});

describe("import.meta.env.VITE_REQFORGE_API_BASE", () => {
  const originalBase = (import.meta.env as Record<string, string | undefined>)
    .VITE_REQFORGE_API_BASE;

  beforeEach(() => {
    (
      import.meta.env as Record<string, string | undefined>
    ).VITE_REQFORGE_API_BASE = originalBase;
  });

  afterEach(() => {
    (
      import.meta.env as Record<string, string | undefined>
    ).VITE_REQFORGE_API_BASE = originalBase;
  });

  it("uses relative URLs by default", async () => {
    const calls = stubFetch(() => json({ status: "ok" }));
    await api.health();
    expect(calls[0]?.url).toBe("/healthz");
  });
});
