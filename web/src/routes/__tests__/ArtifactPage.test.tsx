import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";

import type { ArtifactDetail, ArtifactListing } from "../../api/types";
import { TestQueryProvider } from "../../test-utils";
import { ArtifactPage } from "../ArtifactPage";

function stubFetchByPath(responses: Record<string, unknown>) {
  vi.stubGlobal("fetch", async (input: RequestInfo) => {
    const url = typeof input === "string" ? input : input.toString();
    const match = Object.keys(responses).find((p) => url.endsWith(p));
    if (match === undefined) {
      return new Response(JSON.stringify({ error: "not found" }), {
        status: 404,
        headers: { "content-type": "application/json" },
      });
    }
    return new Response(JSON.stringify(responses[match]), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  });
}

function renderAt(path: string) {
  return render(
    <TestQueryProvider>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route
            path="/projects/:slug/collections/:prefix/artifacts/:name"
            element={<ArtifactPage />}
          />
          <Route path="/artifacts/:uuid" element={<ArtifactPage />} />
        </Routes>
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

const UUID = "0194f6d0-0001-7000-8000-000000000001";

const SAMPLE_DETAIL: ArtifactDetail = {
  name: "REQ-hello",
  projectSlug: "sample",
  collectionPrefix: "REQ",
  uuid: UUID,
  title: "Hello World",
  shape: "content",
  description: "A tiny starter requirement.",
  active: true,
  derived: false,
  createdAt: "2026-04-18T00:00:00Z",
  modifiedAt: "2026-04-18T00:00:00Z",
  tags: ["starter", "demo"],
  links: [
    {
      targetUuid: "0194f6d0-0001-7000-8000-000000000099",
      type: "satisfies",
      hint: {
        projectSlug: "sample",
        collectionPrefix: "REQ",
        artifactName: "REQ-parent",
      },
      resolution: "unresolved",
    },
  ],
  reviewLog: [
    {
      timestamp: "2026-04-18T00:00:00Z",
      reviewer: "alice",
      outcome: "approved",
    },
  ],
  reviewState: {
    state: "approved",
    lastApprovalAt: "2026-04-18T00:00:00Z",
    lastEventAt: "2026-04-18T00:00:00Z",
    lastReviewer: "alice",
    blockingTodos: [],
  },
  body: "# Hello\n\nThis is **markdown**.\n\n- one\n- two\n",
};

const SAMPLE_LISTING: ArtifactListing[] = [
  {
    name: "REQ-hello",
    uuid: UUID,
    title: "Hello World",
    shape: "content",
    active: true,
    reviewState: "approved",
  },
];

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ArtifactPage", () => {
  it("resolves by name via the collection listing and renders the artifact", async () => {
    stubFetchByPath({
      "/api/projects/sample/collections/REQ/artifacts": SAMPLE_LISTING,
      [`/api/artifacts/${UUID}`]: SAMPLE_DETAIL,
    });
    renderAt("/projects/sample/collections/REQ/artifacts/REQ-hello");

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "Hello World" }),
      ).toBeInTheDocument(),
    );

    // Markdown renders headings and list items.
    expect(
      screen.getByRole("heading", { level: 1, name: "Hello" }),
    ).toBeInTheDocument();
    expect(screen.getByText("one")).toBeInTheDocument();
    expect(screen.getByText("two")).toBeInTheDocument();

    // Tags.
    expect(screen.getByText("starter")).toBeInTheDocument();
    expect(screen.getByText("demo")).toBeInTheDocument();

    // Outgoing link renders the target hint. The link-type name
    // appears both as the group heading and on the LinkTypeBadge,
    // so we look for at least one occurrence instead of exactly one.
    expect(screen.getAllByText("satisfies").length).toBeGreaterThan(0);
    expect(screen.getByText(/REQ-parent/)).toBeInTheDocument();

    // Review pane: state badge + review log entry rendering.
    expect(
      screen.getByLabelText(/review state: approved/i),
    ).toBeInTheDocument();
    // "approved" appears multiple times (badge + log entry); at least
    // one occurrence is enough.
    expect(screen.getAllByText("approved").length).toBeGreaterThan(0);
  });

  it("fetches directly when accessed via /artifacts/:uuid", async () => {
    stubFetchByPath({
      [`/api/artifacts/${UUID}`]: SAMPLE_DETAIL,
    });
    renderAt(`/artifacts/${UUID}`);

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "Hello World" }),
      ).toBeInTheDocument(),
    );
  });

  it("shows an alert when the named artifact is missing from the collection listing", async () => {
    stubFetchByPath({
      "/api/projects/sample/collections/REQ/artifacts": [] as ArtifactListing[],
    });
    renderAt("/projects/sample/collections/REQ/artifacts/REQ-ghost");

    await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
    expect(screen.getByText(/REQ-ghost/)).toBeInTheDocument();
  });

  it("renders the blob artifact view with a Download link for blob shapes", async () => {
    const blob = {
      ...SAMPLE_DETAIL,
      shape: "blob" as const,
      body: null,
      blob: {
        byteSize: 12345,
        contentHash: "deadbeef".padEnd(64, "0"),
        mediaType: "application/pdf",
        downloadUrl: `/api/artifacts/${UUID}/blob`,
        thumbnailUrl: `/api/artifacts/${UUID}/thumbnail`,
      },
    };
    stubFetchByPath({
      [`/api/artifacts/${UUID}`]: blob,
    });
    renderAt(`/artifacts/${UUID}`);

    await waitFor(() => {
      const link = screen.getByRole("link", { name: /download/i });
      expect(link).toBeInTheDocument();
      expect(link.getAttribute("href")).toBe(`/api/artifacts/${UUID}/blob`);
    });
    expect(
      screen.getByRole("button", { name: /replace file/i }),
    ).toBeInTheDocument();
  });

  it("renders the URL artifact view with a Check URL now action for URL shapes", async () => {
    const urlArtifact = {
      ...SAMPLE_DETAIL,
      shape: "url" as const,
      body: null,
      url: "https://example.com/spec",
      checkedAt: "2026-04-20T12:00:00Z",
      checkStatus: "ok" as const,
    };
    stubFetchByPath({
      [`/api/artifacts/${UUID}`]: urlArtifact,
    });
    renderAt(`/artifacts/${UUID}`);

    await waitFor(() =>
      expect(
        screen.getByRole("link", { name: "https://example.com/spec" }),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("button", { name: /check url now/i }),
    ).toBeInTheDocument();
    expect(screen.getByText("OK")).toBeInTheDocument();
  });
});
