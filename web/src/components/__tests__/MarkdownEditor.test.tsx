import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MarkdownEditor } from "../MarkdownEditor";

describe("MarkdownEditor", () => {
  it("renders the preview pane with rendered Markdown", () => {
    render(
      <MarkdownEditor value={"# Hello\n\nBody text"} onChange={() => {}} />,
    );
    // Preview side has `prose` styling and renders headings/paragraphs.
    const preview = screen.getByRole("article", { name: /Rendered preview/i });
    expect(preview).toBeInTheDocument();
    expect(preview.querySelector("h1")?.textContent).toBe("Hello");
    expect(preview.textContent).toContain("Body text");
  });

  it("labels the editor pane for screen readers", () => {
    render(
      <MarkdownEditor
        value={""}
        onChange={() => {}}
        ariaLabel="Artifact Markdown"
      />,
    );
    // The CodeMirror contenteditable gets the aria-label.
    expect(screen.getByLabelText(/Artifact Markdown/i)).toBeInTheDocument();
  });
});
