import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import { App } from "./App";
import type { Backlog, Detail, VerifyResponse } from "./types";

afterEach(() => {
  vi.restoreAllMocks();
});

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

/** Route fetch by URL: the list vs a per-id detail. */
function mockRoutes(backlog: Backlog, details: Record<string, Detail> = {}) {
  vi.spyOn(globalThis, "fetch").mockImplementation((input) => {
    const url = typeof input === "string" ? input : (input as Request).url ?? String(input);
    const detailMatch = url.match(/\/api\/requirements\/(.+)$/);
    if (detailMatch) {
      const d = details[decodeURIComponent(detailMatch[1])];
      return Promise.resolve(d ? json(d) : json({ error: "not found" }, 404));
    }
    return Promise.resolve(json(backlog));
  });
}

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
    { id: "REQ001", title: "Login invariant", text: "prose", classification: "formalizable-now", formalization: "drafting", verdict: { status: "holds", basis: "proven", reason: null, fresh: true, stale_reasons: [] } },
    { id: "REQ002", title: null, text: "some prose here", classification: null, formalization: "none", verdict: null },
    { id: "REQ003", title: "A note", text: "prose", classification: "stays-prose", formalization: "none", verdict: { status: "holds", basis: "proven", reason: null, fresh: false, stale_reasons: ["the subject code moved since this verdict (commit abc → def) — re-verify"] } },
  ],
};

function mockBacklog(backlog: Backlog) {
  mockRoutes(backlog);
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

test("clicking a requirement opens its detail with the candidate and read-back", async () => {
  const user = userEvent.setup();
  const detail: Detail = {
    id: "REQ001",
    title: "Login invariant",
    text: "A logged-in user always has a session.",
    revision: "r1",
    stale: false,
    classification: "formalizable-now",
    formalization: "admitted",
    admission: { review: "optional", by: "gg" },
    candidate: "requirement r { category: 1 ... }",
    gate: { status: "passed", warnings: [] },
    readback: "At every state, if the user is logged in then the user has a session.",
    bindings: [{ symbol: "logged_in", category: "code", observable: "login", fidelity: "definitional" }],
    grounding: {
      grounded: true,
      bindings: [
        { symbol: "logged_in", observable: "login", category: "code", resolved: true, summary: "logged_in → `login` resolves to src/lib.rs:1" },
      ],
    },
    verdict: null,
  };
  mockRoutes(SAMPLE, { REQ001: detail });
  render(<App />);

  await user.click(await screen.findByRole("button", { name: "REQ001" }));

  const dialog = await screen.findByRole("dialog");
  expect(within(dialog).getByText(/if the user is logged in/)).toBeInTheDocument();
  expect(within(dialog).getByText(/requirement r \{/)).toBeInTheDocument();
  expect(within(dialog).getByText("login")).toBeInTheDocument();
  // The live grounding report renders its grounded status and per-binding read-back.
  expect(within(dialog).getByText("grounded")).toBeInTheDocument();
  expect(within(dialog).getByText(/resolves to src\/lib\.rs:1/)).toBeInTheDocument();
});

test("changing a row's triage bucket writes and reconciles to the server state", async () => {
  const user = userEvent.setup();
  // REQ002 (untriaged) becomes stays-prose in the authoritative response.
  const after: Backlog = {
    coverage: { ...SAMPLE.coverage, untriaged: 0, stays_prose: 2 },
    items: SAMPLE.items.map((i) => (i.id === "REQ002" ? { ...i, classification: "stays-prose" } : i)),
  };
  const fetchSpy = vi.spyOn(globalThis, "fetch").mockImplementation((input) => {
    const url = typeof input === "string" ? input : (input as Request).url;
    if (url.endsWith("/triage")) return Promise.resolve(json(after));
    return Promise.resolve(json(SAMPLE));
  });
  render(<App />);
  await screen.findByText("REQ001");

  const select = screen.getByLabelText("Triage bucket for REQ002") as HTMLSelectElement;
  await user.selectOptions(select, "stays-prose");

  await waitFor(() => expect(select.value).toBe("stays-prose"));
  expect(fetchSpy).toHaveBeenCalledWith(
    "/api/requirements/REQ002/triage",
    expect.objectContaining({ method: "POST" }),
  );
});

test("the backlog surfaces each item's stored verdict, marking a drifted one stale (REQ039)", async () => {
  mockBacklog(SAMPLE);
  render(<App />);
  await screen.findByText("REQ001");

  const table = screen.getByRole("table");
  // REQ001's stored holds is fresh — no stale marker; REQ003's holds has drifted — marked stale.
  const holds = within(table).getAllByText("holds");
  expect(holds).toHaveLength(2);
  const stale = within(table).getByText("⟳ stale");
  expect(stale).toBeInTheDocument();
  expect(stale).toHaveAttribute("title", expect.stringContaining("subject code moved"));
  // REQ002 has never been verified.
  expect(within(table).getByText("not verified")).toBeInTheDocument();
});

test("clicking Verify runs the ensemble and renders the verdict with per-engine evidence", async () => {
  const user = userEvent.setup();
  const detail: Detail = {
    id: "REQ001",
    title: "Login invariant",
    text: "A logged-in user always has a session.",
    revision: "r1",
    stale: false,
    classification: "formalizable-now",
    formalization: "admitted",
    admission: { review: "optional", by: "gg" },
    candidate: "requirement r { category: 1 ... }",
    gate: { status: "passed", warnings: [] },
    readback: "At every state...",
    bindings: [],
    grounding: null,
    verdict: null,
  };
  const verdict: VerifyResponse = {
    state: "verdict",
    stale: false,
    verdict: {
      id: "REQ001",
      status: "holds",
      basis: "proven",
      reason: null,
      witness: null,
      detail: [],
      evidence: [
        { engine: "Creusot", status: "holds", basis: "proven", witness: null, detail: [] },
        { engine: "Kani", status: "unknown", basis: null, witness: null, detail: ["harness would not compile"] },
      ],
      provenance: { requirement_revision: "r1", subject_commit: "abc123", tool_version: "0.0.1" },
    },
  };
  // Route verify (POST) before the generic detail matcher, since both share the id prefix.
  vi.spyOn(globalThis, "fetch").mockImplementation((input) => {
    const url = typeof input === "string" ? input : (input as Request).url ?? String(input);
    if (url.endsWith("/verify")) return Promise.resolve(json(verdict));
    if (/\/api\/requirements\/REQ001$/.test(url)) return Promise.resolve(json(detail));
    return Promise.resolve(json(SAMPLE));
  });
  render(<App />);

  await user.click(await screen.findByRole("button", { name: "REQ001" }));
  const dialog = await screen.findByRole("dialog");
  await user.click(within(dialog).getByRole("button", { name: "Verify" }));

  // The aggregate polarity and each engine's own result render (aggregate + Creusot both "holds").
  expect(await within(dialog).findAllByText("holds")).toHaveLength(2);
  expect(within(dialog).getByText("Creusot")).toBeInTheDocument();
  expect(within(dialog).getByText("Kani")).toBeInTheDocument();
  expect(within(dialog).getByText("harness would not compile")).toBeInTheDocument();
});

test("a failed triage write rolls back and surfaces an error", async () => {
  const user = userEvent.setup();
  vi.spyOn(globalThis, "fetch").mockImplementation((input) => {
    const url = typeof input === "string" ? input : (input as Request).url;
    if (url.endsWith("/triage")) return Promise.resolve(json({ error: "disk full" }, 409));
    return Promise.resolve(json(SAMPLE));
  });
  render(<App />);
  await screen.findByText("REQ001");

  const select = screen.getByLabelText("Triage bucket for REQ002") as HTMLSelectElement;
  await user.selectOptions(select, "stays-prose");

  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("disk full"));
  // Rolled back to the original untriaged value.
  expect(select.value).toBe("");
});
