import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ArtifactDetail, BlobDetail } from "../../../api/types";
import { BlobArtifactView } from "../BlobArtifactView";

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

function baseArtifact(blob: BlobDetail): ArtifactDetail {
  return {
    name: "DES-spec",
    projectSlug: "sample",
    collectionPrefix: "DES",
    uuid: "0194f6d0-0000-7000-8000-000000000001",
    title: "Design spec",
    shape: "blob",
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
    blob,
  };
}

describe("BlobArtifactView", () => {
  it("renders an image element inline for PNG media types", () => {
    const { container } = render(
      <BlobArtifactView
        artifact={baseArtifact({
          byteSize: 1024,
          contentHash: "a".repeat(64),
          mediaType: "image/png",
          downloadUrl: "/api/artifacts/uuid/blob",
          thumbnailUrl: "/api/artifacts/uuid/thumbnail",
        })}
      />,
      { wrapper },
    );
    const img = container.querySelector('img[src="/api/artifacts/uuid/blob"]');
    expect(img).not.toBeNull();
  });

  it("renders a PDF via iframe inline", () => {
    const { container } = render(
      <BlobArtifactView
        artifact={baseArtifact({
          byteSize: 2048,
          contentHash: "b".repeat(64),
          mediaType: "application/pdf",
          downloadUrl: "/api/artifacts/uuid/blob",
          thumbnailUrl: "/api/artifacts/uuid/thumbnail",
        })}
      />,
      { wrapper },
    );
    const frame = container.querySelector("iframe");
    expect(frame).not.toBeNull();
    expect(frame?.getAttribute("src")).toBe("/api/artifacts/uuid/blob");
  });

  it("shows a thumbnail preview for Office media types", () => {
    render(
      <BlobArtifactView
        artifact={baseArtifact({
          byteSize: 4096,
          contentHash: "c".repeat(64),
          mediaType:
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
          downloadUrl: "/api/artifacts/uuid/blob",
          thumbnailUrl: "/api/artifacts/uuid/thumbnail",
        })}
      />,
      { wrapper },
    );
    const img = screen.getByAltText("thumbnail");
    expect(img.getAttribute("src")).toBe("/api/artifacts/uuid/thumbnail");
  });

  it("shows the icon-only fallback when the media type is unknown", () => {
    render(
      <BlobArtifactView
        artifact={baseArtifact({
          byteSize: 5000,
          contentHash: "d".repeat(64),
          mediaType: "application/x-msdownload",
          downloadUrl: "/api/artifacts/uuid/blob",
          thumbnailUrl: "/api/artifacts/uuid/thumbnail",
        })}
      />,
      { wrapper },
    );
    expect(
      screen.getByText(/ReqForge doesn't have a thumbnailer/i),
    ).toBeInTheDocument();
  });

  it("always renders a Download link pointing at the download URL", () => {
    render(
      <BlobArtifactView
        artifact={baseArtifact({
          byteSize: 10,
          contentHash: "e".repeat(64),
          mediaType: "text/plain",
          downloadUrl: "/api/artifacts/uuid/blob",
          thumbnailUrl: "/api/artifacts/uuid/thumbnail",
        })}
      />,
      { wrapper },
    );
    const link = screen.getByRole("link", { name: /download/i });
    expect(link.getAttribute("href")).toBe("/api/artifacts/uuid/blob");
  });
});
