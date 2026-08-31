import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { BrowsePage } from "../BrowsePage";

interface FetchCall {
  url: string;
  method: string;
}

function stubFetch(handler: (url: string, method: string) => unknown) {
  const calls: FetchCall[] = [];
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";
    calls.push({ url, method });
    const body = handler(url, method);
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  });
  return calls;
}

function renderPage() {
  return render(
    <TestQueryProvider>
      <MemoryRouter initialEntries={["/browse"]}>
        <BrowsePage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

function samplePane(
  prefix: string,
  name: string,
  artifactCount: number,
  extras: { nameVariants?: string[]; tags?: string[] } = {},
) {
  const artifacts = Array.from({ length: artifactCount }, (_, i) => ({
    uuid: `u-${prefix}-${i}`,
    projectSlug: "sample",
    collectionPrefix: prefix,
    artifactName: `${prefix}-${i.toString().padStart(2, "0")}`,
    title: `Item ${i}`,
    shape: "content" as const,
    active: true,
    reviewState: "never-reviewed" as const,
    tags: extras.tags ?? [],
  }));
  return {
    prefix,
    name,
    nameVariants: extras.nameVariants,
    totalArtifacts: artifactCount,
    artifacts,
  };
}

describe("BrowsePage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders one collapsible pane per Collection prefix with the correct headline", async () => {
    stubFetch((url) => {
      if (url.includes("/api/browse")) {
        return {
          scope: { kind: "system" },
          totalPanes: 2,
          totalArtifacts: 3,
          panes: [
            samplePane("DES", "Design Documents", 1),
            samplePane("REQ", "Requirements", 2),
          ],
        };
      }
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByTestId("browse-pane-DES")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("browse-pane-REQ")).toBeInTheDocument();
    // Panes start collapsed — rows aren't rendered until
    // expanded.
    expect(screen.queryByTestId("browse-row-REQ-00")).not.toBeInTheDocument();
    const reqPaneHeader = screen
      .getByTestId("browse-pane-REQ")
      .querySelector("button");
    expect(reqPaneHeader).not.toBeNull();
    await userEvent.click(reqPaneHeader!);
    expect(await screen.findByTestId("browse-row-REQ-00")).toBeInTheDocument();
    expect(screen.getByTestId("browse-row-REQ-01")).toBeInTheDocument();
  });

  it("renders a name-drift pill when the pane has nameVariants", async () => {
    stubFetch((url) => {
      if (url.includes("/api/browse")) {
        return {
          scope: { kind: "system" },
          totalPanes: 1,
          totalArtifacts: 2,
          panes: [
            samplePane("REQ", "Regulations", 2, {
              nameVariants: ["Requirements"],
            }),
          ],
        };
      }
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();
    const pill = await screen.findByTestId("browse-pane-REQ-variants");
    expect(pill).toHaveTextContent(/name drift/i);
    expect(pill).toHaveAttribute(
      "title",
      expect.stringContaining("Requirements"),
    );
  });

  it("in-pane title filter narrows client-side without an additional fetch", async () => {
    const calls = stubFetch((url) => {
      if (url.includes("/api/browse")) {
        return {
          scope: { kind: "system" },
          totalPanes: 1,
          totalArtifacts: 3,
          panes: [samplePane("REQ", "Requirements", 3)],
        };
      }
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();
    const header = (await screen.findByTestId("browse-pane-REQ")).querySelector(
      "button",
    );
    await userEvent.click(header!);
    await screen.findByTestId("browse-row-REQ-00");

    const beforeCalls = calls.length;
    const filter = screen.getByLabelText(/filter req artifacts/i);
    await userEvent.type(filter, "item 1");
    // Only the matching row survives; no extra network request fired.
    expect(calls.length).toBe(beforeCalls);
    expect(screen.getByTestId("browse-row-REQ-01")).toBeInTheDocument();
    expect(screen.queryByTestId("browse-row-REQ-00")).not.toBeInTheDocument();
    expect(screen.queryByTestId("browse-row-REQ-02")).not.toBeInTheDocument();
  });

  it("toggling a review-state chip threads the filter into /api/browse", async () => {
    const calls = stubFetch((url) => {
      if (url.includes("/api/browse")) {
        return {
          scope: { kind: "system" },
          totalPanes: 0,
          totalArtifacts: 0,
          panes: [],
        };
      }
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();
    const chip = await screen.findByRole("button", { name: /^approved$/ });
    await userEvent.click(chip);
    await waitFor(() => {
      expect(
        calls.some(
          (c) =>
            c.url.includes("/api/browse") &&
            c.url.includes("reviewState=approved"),
        ),
      ).toBe(true);
    });
  });

  it("shows an empty-filters message when the response has no panes", async () => {
    stubFetch((url) => {
      if (url.includes("/api/browse")) {
        return {
          scope: { kind: "system" },
          totalPanes: 0,
          totalArtifacts: 0,
          panes: [],
        };
      }
      if (url.includes("/api/projects")) return [];
      return {};
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByText(/no artifacts match the current filters/i),
      ).toBeInTheDocument(),
    );
  });
});
