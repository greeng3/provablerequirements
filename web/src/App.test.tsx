import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import { App } from "./App";
import type { Backlog } from "./types";

afterEach(() => {
  vi.restoreAllMocks();
});

const SAMPLE: Backlog = {
  coverage: {
    discovered: 3,
    untriaged: 1,
    formalizable_now: 1,
    falsifiable_only: 0,
    stays_prose: 1,
    drafting: 1,
    formalized: 0,
    verified: 0,
  },
  items: [
    { id: "REQ001", title: "Login invariant", text: "prose", classification: "formalizable-now", formalization: "drafting" },
    { id: "REQ002", title: null, text: "some prose here", classification: null, formalization: "none" },
    { id: "REQ003", title: "A note", text: "prose", classification: "stays-prose", formalization: "none" },
  ],
};

function mockBacklog(backlog: Backlog) {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(JSON.stringify(backlog), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    }),
  );
}

test("renders the coverage funnel and one row per requirement", async () => {
  mockBacklog(SAMPLE);
  render(<App />);

  expect(await screen.findByText("REQ001")).toBeInTheDocument();
  expect(screen.getByText("REQ002")).toBeInTheDocument();
  expect(screen.getByText("REQ003")).toBeInTheDocument();
  // Coverage summary reflects the payload.
  expect(screen.getByText("3 discovered")).toBeInTheDocument();
});

test("the funnel tabs filter the list", async () => {
  const user = userEvent.setup();
  mockBacklog(SAMPLE);
  render(<App />);
  await screen.findByText("REQ001");

  await user.click(screen.getByRole("tab", { name: "Untriaged" }));

  const table = screen.getByRole("table");
  expect(within(table).getByText("REQ002")).toBeInTheDocument();
  expect(within(table).queryByText("REQ001")).not.toBeInTheDocument();
  expect(within(table).queryByText("REQ003")).not.toBeInTheDocument();
});

test("surfaces the backend error message when the subject is not adopted", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(JSON.stringify({ error: "no companion tree found — run `provreq init` first" }), {
      status: 409,
      headers: { "Content-Type": "application/json" },
    }),
  );
  render(<App />);

  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent("provreq init"),
  );
});
