import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ArtifactDiffResponse } from "../../../api/types";
import { DiffView } from "../DiffView";

function contentResponse(
  lines: Array<{ kind: "same" | "added" | "removed"; text: string }>,
): ArtifactDiffResponse {
  return {
    shape: "content",
    fromLabel: "abc1234567",
    toLabel: "working tree",
    diff: { shape: "content", lines },
  };
}

describe("DiffView", () => {
  it("renders the from → to labels", () => {
    render(
      <DiffView
        response={contentResponse([{ kind: "same", text: "hello" }])}
      />,
    );
    expect(screen.getByText("abc1234567")).toBeInTheDocument();
    expect(screen.getByText("working tree")).toBeInTheDocument();
  });

  it("surfaces fallbackReason in an alert banner when present", () => {
    const response: ArtifactDiffResponse = {
      ...contentResponse([]),
      fallbackReason: "shallow clone",
    };
    render(<DiffView response={response} />);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("shallow clone");
    expect(alert.textContent).toContain("approval snapshot");
  });

  it("renders 'No line-level changes.' when all lines are same", () => {
    render(
      <DiffView
        response={contentResponse([{ kind: "same", text: "unchanged" }])}
      />,
    );
    expect(screen.getByText(/no line-level changes/i)).toBeInTheDocument();
  });

  it("marks added and removed lines with the right sentinel characters", () => {
    render(
      <DiffView
        response={contentResponse([
          { kind: "removed", text: "old line" },
          { kind: "added", text: "new line" },
        ])}
      />,
    );
    expect(screen.getByText("old line")).toBeInTheDocument();
    expect(screen.getByText("new line")).toBeInTheDocument();
    expect(screen.getByText("-")).toBeInTheDocument();
    expect(screen.getByText("+")).toBeInTheDocument();
  });

  it("renders a blob diff with before/after cards", () => {
    const response: ArtifactDiffResponse = {
      shape: "blob",
      fromLabel: "aaaaaaaaaa",
      toLabel: "bbbbbbbbbb",
      diff: {
        shape: "blob",
        before: {
          byteSize: 1024,
          contentHash: "a".repeat(64),
          mediaType: "application/pdf",
          downloadUrl: "/api/artifacts/u/blob?at=aaaaaaaaaa",
        },
        after: {
          byteSize: 2048,
          contentHash: "b".repeat(64),
          mediaType: "application/pdf",
          downloadUrl: "/api/artifacts/u/blob?at=bbbbbbbbbb",
        },
      },
    };
    render(<DiffView response={response} />);
    expect(screen.getByText("Before")).toBeInTheDocument();
    expect(screen.getByText("After")).toBeInTheDocument();
    const links = screen.getAllByRole("link", { name: /download/i });
    expect(links.length).toBe(2);
  });

  it("renders a URL diff with external-content disclaimer", () => {
    const response: ArtifactDiffResponse = {
      shape: "url",
      fromLabel: "from",
      toLabel: "to",
      diff: {
        shape: "url",
        before: "https://old.example.com",
        after: "https://new.example.com",
        note: "External content disclaimer lives here.",
      },
    };
    render(<DiffView response={response} />);
    expect(
      screen.getByText(/external content disclaimer/i),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "https://new.example.com" }),
    ).toBeInTheDocument();
  });

  it("says '(not present at this commit)' when a blob side is absent", () => {
    const response: ArtifactDiffResponse = {
      shape: "blob",
      fromLabel: "f",
      toLabel: "t",
      diff: {
        shape: "blob",
        before: undefined,
        after: {
          byteSize: 10,
          contentHash: "a".repeat(64),
          mediaType: "image/png",
          downloadUrl: "/api/artifacts/u/blob",
        },
      },
    };
    render(<DiffView response={response} />);
    expect(screen.getByText(/not present at this commit/i)).toBeInTheDocument();
  });
});
