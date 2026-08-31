import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SearchSnippet, parseMarkSegments } from "../SearchSnippet";

describe("parseMarkSegments", () => {
  it("splits plain text + match segments on literal <mark> tokens", () => {
    const segs = parseMarkSegments("Safety <mark>reactor</mark> envelope");
    expect(segs).toEqual([
      { kind: "plain", text: "Safety " },
      { kind: "match", text: "reactor" },
      { kind: "plain", text: " envelope" },
    ]);
  });

  it("decodes the five XML entities Tantivy's snippet emits", () => {
    const segs = parseMarkSegments(
      "a &amp; b &lt;c&gt; &quot;d&quot; &#x27;e&#x27;",
    );
    expect(segs).toEqual([{ kind: "plain", text: "a & b <c> \"d\" 'e'" }]);
  });

  it("treats an unclosed <mark> as plain text so a malformed payload stays inert", () => {
    const segs = parseMarkSegments("foo <mark>bar baz");
    expect(segs).toEqual([
      { kind: "plain", text: "foo " },
      { kind: "plain", text: "bar baz" },
    ]);
  });
});

describe("SearchSnippet component", () => {
  it("renders <mark> segments as DOM mark elements — never as HTML", () => {
    render(
      <SearchSnippet snippet="The <mark>reactor</mark> vessel shall satisfy." />,
    );
    const marks = screen.getAllByTestId("search-snippet-mark");
    expect(marks).toHaveLength(1);
    expect(marks[0]).toHaveTextContent("reactor");
  });

  it("renders a would-be injection payload as inert text when it lacks mark tokens", () => {
    // A hostile `body` excerpt after backend escaping would
    // arrive as `&lt;script&gt;...` — no `<mark>` token, no
    // dangerouslySetInnerHTML, so nothing executes.
    const { container } = render(
      <SearchSnippet snippet="&lt;script&gt;alert(1)&lt;/script&gt;" />,
    );
    // No script element, no mark element — just text.
    expect(container.querySelector("script")).toBeNull();
    expect(container).toHaveTextContent("<script>alert(1)</script>");
  });
});
