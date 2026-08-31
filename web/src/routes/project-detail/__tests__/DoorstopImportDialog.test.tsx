import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { DoorstopImportDialog } from "../DoorstopImportDialog";

interface FetchCall {
  url: string;
  method: string;
  body?: string;
}

function stubFetch(
  handler: (
    url: string,
    method: string,
    body?: string,
  ) => { status: number; body: unknown },
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

function mount() {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <DoorstopImportDialog projectSlug="sample" onClose={() => {}} />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

function samplePlan(
  override: Partial<{
    collections: unknown[];
    prefixCollisions: unknown[];
    unresolvedLinks: unknown[];
    warnings: string[];
  }> = {},
) {
  return {
    importRunAt: "2026-04-22T00:00:00Z",
    collections: override.collections ?? [
      {
        prefix: "REQ",
        name: "REQ (imported from doorstop)",
        directoryName: "req",
        sourceMarkerPath: "/p/doorstop/req/.doorstop.yml",
        importNotes: {},
        artifacts: [
          {
            uuid: "a",
            name: "REQ-001",
            originalUid: "REQ-001",
            title: "Test",
            body: "body",
            active: true,
            derived: false,
            outlineLevel: null,
            links: [],
            tags: [],
            refDisposition: { kind: "none" },
            syntheticReview: null,
            legacyExtensions: {},
            sourcePath: "/p/doorstop/req/REQ-001.yml",
          },
        ],
        emptyWarning: null,
      },
    ],
    prefixCollisions: override.prefixCollisions ?? [],
    unresolvedLinks: override.unresolvedLinks ?? [],
    warnings: override.warnings ?? [],
  };
}

function sampleReport() {
  return {
    projectSlug: "sample",
    source: "doorstop",
    importRunAt: "2026-04-22T00:00:00Z",
    collections: [
      {
        prefix: "REQ",
        name: "REQ (imported from doorstop)",
        directoryName: "req",
        artifactCount: 1,
        syntheticReviewCount: 0,
        legacyPreservedCount: 0,
        derivesFromLinkCount: 0,
        urlArtifactCount: 0,
        sourceMarkerPath: "/p/doorstop/req/.doorstop.yml",
      },
    ],
    totals: {
      collectionsCreated: 1,
      artifactsImported: 1,
      derivesFromLinks: 0,
      urlArtifacts: 0,
      citesLinks: 0,
      legacyRefs: 0,
      syntheticReviewEntries: 0,
      legacyPreservedFields: 0,
      unresolvedLinkCount: 0,
    },
    refDispositions: [],
    unresolvedLinks: [],
    prefixCollisions: [],
    warnings: [],
  };
}

describe("DoorstopImportDialog", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("runs preview → import → report and reaches the success panel", async () => {
    const calls = stubFetch((url, method) => {
      if (url.endsWith("/doorstop/preview") && method === "POST") {
        return { status: 200, body: samplePlan() };
      }
      if (url.endsWith("/doorstop/import") && method === "POST") {
        return { status: 200, body: sampleReport() };
      }
      return { status: 200, body: {} };
    });
    mount();
    await userEvent.type(
      screen.getByTestId("doorstop-source-input"),
      "doorstop",
    );
    await userEvent.click(screen.getByTestId("doorstop-preview-button"));
    await waitFor(() =>
      expect(screen.getByTestId("doorstop-plan-summary")).toBeInTheDocument(),
    );
    expect(
      screen.getByTestId("doorstop-plan-collection-REQ"),
    ).toBeInTheDocument();
    // Preview call was made with the right body.
    const previewCall = calls.find((c) => c.url.endsWith("/doorstop/preview"));
    expect(previewCall?.body).toContain('"source":"doorstop"');

    await userEvent.click(screen.getByTestId("doorstop-import-button"));
    await waitFor(() =>
      expect(screen.getByTestId("doorstop-report-panel")).toBeInTheDocument(),
    );
    // Success panel shows totals.
    expect(screen.getByText(/Import complete\./)).toBeInTheDocument();
  });

  it("surfaces prefix collisions in the preview panel and disables the Import button", async () => {
    stubFetch((url) => {
      if (url.endsWith("/doorstop/preview")) {
        return {
          status: 200,
          body: samplePlan({
            prefixCollisions: [
              {
                prefix: "REQ",
                existingCollectionDirectory: "requirements",
                doorstopMarkerPath: "/p/doorstop/req/.doorstop.yml",
              },
            ],
          }),
        };
      }
      return { status: 200, body: {} };
    });
    mount();
    await userEvent.type(
      screen.getByTestId("doorstop-source-input"),
      "doorstop",
    );
    await userEvent.click(screen.getByTestId("doorstop-preview-button"));
    const banner = await screen.findByTestId("doorstop-collision-banner");
    expect(banner).toHaveTextContent(/prefix collision/i);
    expect(banner).toHaveTextContent(/REQ/);
    expect(screen.getByTestId("doorstop-import-button")).toBeDisabled();
  });

  it("surfaces the backend's 409 body when the import endpoint refuses after preview", async () => {
    // Preview allows the plan (no collision visible to the
    // preview) but the import endpoint returns 409 because a
    // concurrent write raced in. The dialog parses the
    // structured ApiError body and renders each collision.
    stubFetch((url, method) => {
      if (url.endsWith("/doorstop/preview")) {
        return { status: 200, body: samplePlan() };
      }
      if (url.endsWith("/doorstop/import") && method === "POST") {
        return {
          status: 409,
          body: {
            error: "prefix collision — resolve and re-run",
            collisions: [
              {
                prefix: "REQ",
                existingCollectionDirectory: "requirements",
                doorstopMarkerPath: "/p/doorstop/req/.doorstop.yml",
              },
            ],
          },
        };
      }
      return { status: 200, body: {} };
    });
    mount();
    await userEvent.type(
      screen.getByTestId("doorstop-source-input"),
      "doorstop",
    );
    await userEvent.click(screen.getByTestId("doorstop-preview-button"));
    await screen.findByTestId("doorstop-plan-summary");
    await userEvent.click(screen.getByTestId("doorstop-import-button"));
    await waitFor(() =>
      expect(
        screen.getByText(/prefix collision — resolve and re-run/i),
      ).toBeInTheDocument(),
    );
    // Still on the preview panel, not the report panel.
    expect(
      screen.queryByTestId("doorstop-report-panel"),
    ).not.toBeInTheDocument();
  });

  it("offers Analyze for links after a successful import and POSTs the analyze endpoint on click", async () => {
    const providersBody = {
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
    const calls = stubFetch((url, method) => {
      if (url.endsWith("/api/llm/providers")) {
        return { status: 200, body: providersBody };
      }
      if (url.endsWith("/doorstop/preview") && method === "POST") {
        return { status: 200, body: samplePlan() };
      }
      if (url.endsWith("/doorstop/import") && method === "POST") {
        return { status: 200, body: sampleReport() };
      }
      if (url.endsWith("/suggestions/links/analyze") && method === "POST") {
        return {
          status: 200,
          body: {
            kind: "ok",
            suggestions: [],
            servedByIndex: 0,
            servedBy: "openaiCompatible/gpt-4o-mini",
          },
        };
      }
      return { status: 200, body: {} };
    });
    const onClose = vi.fn();
    render(
      <TestQueryProvider>
        <MemoryRouter>
          <DoorstopImportDialog projectSlug="sample" onClose={onClose} />
        </MemoryRouter>
      </TestQueryProvider>,
    );

    await userEvent.type(
      screen.getByTestId("doorstop-source-input"),
      "doorstop",
    );
    await userEvent.click(screen.getByTestId("doorstop-preview-button"));
    await waitFor(() =>
      expect(screen.getByTestId("doorstop-plan-summary")).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByTestId("doorstop-import-button"));
    const analyzeBtn = await screen.findByTestId(
      "doorstop-analyze-links-button",
    );
    await userEvent.click(analyzeBtn);

    await waitFor(() => {
      const analyzeCall = calls.find(
        (c) =>
          c.method === "POST" && c.url.endsWith("/suggestions/links/analyze"),
      );
      expect(analyzeCall).toBeDefined();
    });
    // onClose fires after the analyze mutation settles so the
    // operator lands back on the project page (where the
    // Suggested Links tab can show the freshly-written results).
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("surfaces a 400 from the preview endpoint as a readable error message", async () => {
    stubFetch((url) => {
      if (url.endsWith("/doorstop/preview")) {
        return {
          status: 400,
          body: {
            error: "source must be a forward-slash project-root-relative path",
          },
        };
      }
      return { status: 200, body: {} };
    });
    mount();
    await userEvent.type(
      screen.getByTestId("doorstop-source-input"),
      "../outside",
    );
    await userEvent.click(screen.getByTestId("doorstop-preview-button"));
    // The rose-600 alert paragraph renders the ApiError
    // toString, which includes the server-provided 400
    // detail. findByRole with role="alert" is the stable
    // anchor (the text layout can change).
    const alert = await screen.findByRole("alert");
    expect(alert.textContent ?? "").toMatch(/project-root-relative/i);
    // Still on the source panel.
    expect(screen.getByTestId("doorstop-source-input")).toBeInTheDocument();
  });
});
