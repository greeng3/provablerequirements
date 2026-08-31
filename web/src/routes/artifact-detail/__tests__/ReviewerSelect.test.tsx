import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ReviewerSelect } from "../ReviewerSelect";
import { TestQueryProvider } from "../../../test-utils";

function installReviewersStub(body: unknown) {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = new URL(input.toString(), "http://localhost");
    if (url.pathname === "/api/reviewers") {
      return new Response(JSON.stringify(body), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    return new Response("[]", { status: 200 });
  });
}

function TestHarness({ initial = "" }: { initial?: string }) {
  const [value, setValue] = useHostState(initial);
  return (
    <>
      <ReviewerSelect projectSlug="sample" value={value} onChange={setValue} />
      <span data-testid="current">{value}</span>
    </>
  );
}

// Tiny hook indirection so the tests can read the current value.
import { useState as useHostState } from "react";

function renderHarness(initial?: string) {
  return render(
    <TestQueryProvider>
      <TestHarness initial={initial ?? ""} />
    </TestQueryProvider>,
  );
}

describe("ReviewerSelect", () => {
  afterEach(() => vi.restoreAllMocks());

  it("pre-populates with the git default when no value is set", async () => {
    installReviewersStub({
      gitDefault: "Alice",
      persisted: [],
      session: [],
    });
    renderHarness();
    await waitFor(() =>
      expect(screen.getByTestId("current")).toHaveTextContent("Alice"),
    );
  });

  it("groups options into default / session / persisted", async () => {
    installReviewersStub({
      gitDefault: "Alice",
      persisted: ["Bob"],
      session: ["Carol"],
    });
    renderHarness();
    await waitFor(() =>
      expect(screen.getByRole("option", { name: "Alice" })).toBeInTheDocument(),
    );
    expect(screen.getByRole("option", { name: "Carol" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Bob" })).toBeInTheDocument();
  });

  it("switches to the free-text input when 'Type a new reviewer…' is picked", async () => {
    installReviewersStub({ gitDefault: "Alice", persisted: [], session: [] });
    const user = userEvent.setup();
    renderHarness("Alice");
    await waitFor(() =>
      expect(screen.getByRole("option", { name: "Alice" })).toBeInTheDocument(),
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: /reviewer identity/i }),
      "__custom__",
    );
    const input = await screen.findByPlaceholderText(/new reviewer identity/i);
    await user.clear(input);
    await user.type(input, "Dana");
    expect(screen.getByTestId("current")).toHaveTextContent("Dana");
  });
});
