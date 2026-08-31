import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ArtifactDetail, UrlCheckStatus } from "../../../api/types";
import { UrlArtifactView } from "../UrlArtifactView";

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

function urlArtifact(
  url: string,
  status?: UrlCheckStatus,
  checkedAt?: string,
): ArtifactDetail {
  return {
    name: "REF-example",
    projectSlug: "sample",
    collectionPrefix: "REF",
    uuid: "0194f6d0-0000-7000-8000-000000000002",
    title: "External ref",
    shape: "url",
    description: null,
    active: true,
    derived: false,
    createdAt: "2026-04-18T00:00:00Z",
    modifiedAt: "2026-04-18T00:00:00Z",
    tags: [],
    links: [],
    reviewLog: [],
    reviewState: {
      state: "neverReviewed",
      blockingTodos: [],
    },
    body: null,
    url,
    checkStatus: status,
    checkedAt,
  };
}

describe("UrlArtifactView", () => {
  it("renders the URL as a new-tab link with safe rel attrs", () => {
    render(
      <UrlArtifactView artifact={urlArtifact("https://example.com/spec")} />,
      { wrapper },
    );
    const link = screen.getByRole("link", {
      name: "https://example.com/spec",
    });
    expect(link.getAttribute("target")).toBe("_blank");
    expect(link.getAttribute("rel")).toContain("noopener");
  });

  it("shows a green OK pill for 'ok' checkStatus", () => {
    render(
      <UrlArtifactView
        artifact={urlArtifact(
          "https://example.com",
          "ok",
          "2026-04-20T12:00:00Z",
        )}
      />,
      { wrapper },
    );
    expect(screen.getByText("OK")).toBeInTheDocument();
  });

  it("shows a rose pill for 'not-found'", () => {
    render(
      <UrlArtifactView
        artifact={urlArtifact(
          "https://example.com/404",
          "not-found",
          "2026-04-20T12:00:00Z",
        )}
      />,
      { wrapper },
    );
    expect(screen.getByText(/404 not found/i)).toBeInTheDocument();
  });

  it("says 'Never checked' when the artifact has no checkStatus", () => {
    render(<UrlArtifactView artifact={urlArtifact("https://example.com")} />, {
      wrapper,
    });
    expect(screen.getByText(/never checked/i)).toBeInTheDocument();
  });

  it("disables the check button when the URL is empty", () => {
    render(<UrlArtifactView artifact={urlArtifact("")} />, { wrapper });
    const button = screen.getByRole("button", { name: /check url now/i });
    expect(button).toBeDisabled();
  });
});
