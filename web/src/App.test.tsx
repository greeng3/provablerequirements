import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import { App } from "./App";
import type { Backlog, Detail, EngineReport, VerifyResponse } from "./types";

afterEach(() => {
  vi.restoreAllMocks();
});

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

/** Every engine healthy — the background the engine-fault tests vary from. */
const ENGINES: EngineReport[] = [
  { category: "1", name: "Kani", state: "available", detail: "available (0.67.0)", reason: null },
  { category: "2a", name: "TLC (TLA+)", state: "available", detail: "available (2.19)", reason: null },
];

/** Route fetch by URL: the engine probe, the list, or a per-id detail. */
function mockRoutes(
  backlog: Backlog,
  details: Record<string, Detail> = {},
  engines: EngineReport[] = ENGINES,
) {
  vi.spyOn(globalThis, "fetch").mockImplementation((input) => {
    const url = typeof input === "string" ? input : (input as Request).url ?? String(input);
    if (url.endsWith("/api/engines")) return Promise.resolve(json({ engines }));
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
    stale: 1,
  },
  items: [
    { id: "REQ001", title: "Login invariant", text: "prose", classification: "formalizable-now", classified_by: "classified", formalization: "drafting", verdict: { status: "holds", basis: "proven", reason: null, detail: [], witness: null, evidence: [], fresh: true, stale_reasons: [], environment: "declared `lab-2`; Kani 0.67.0" } },
    { id: "REQ002", title: null, text: "some prose here", classification: null, classified_by: null, formalization: "none", verdict: null },
    // A seeded `stays-prose`: the exact pair #180 is about — same bucket as a judged one, worth less.
    { id: "REQ003", title: "A note", text: "prose", classification: "stays-prose", classified_by: "seeded", formalization: "none", verdict: { status: "holds", basis: "proven", reason: null, detail: [], witness: null, evidence: [], fresh: false, stale_reasons: ["the subject code moved since this verdict (commit abc → def) — re-verify"], environment: null } },
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
  // The living-loop re-verify tally is surfaced as its own funnel stat (REQ043).
  const staleStat = screen.getByText("stale").closest("div");
  expect(within(staleStat as HTMLElement).getByText("1")).toBeInTheDocument();
});

// Verifies: #180 — the browser distinguishes a bucket a classifier judged from one seeded because
// nothing could. #172 landed this on the record, the CLI and the API row and stopped there, so the
// one surface built for reading a whole backlog at once showed the two identically. Only the
// origins worth less than the bucket looks are annotated: a judgement carries no note, or the note
// would be decoration rather than a signal.
test("a seeded classification is annotated in the backlog, a judged one is not", async () => {
  mockBacklog(SAMPLE);
  render(<App />);
  await screen.findByText("REQ001");

  const table = screen.getByRole("table");
  const seeded = within(table).getByText("REQ003").closest("tr") as HTMLElement;
  expect(within(seeded).getByText("seeded — no classifier ran")).toBeInTheDocument();

  const judged = within(table).getByText("REQ001").closest("tr") as HTMLElement;
  expect(within(judged).queryByText(/no classifier ran/)).not.toBeInTheDocument();
  const untriaged = within(table).getByText("REQ002").closest("tr") as HTMLElement;
  expect(within(untriaged).queryByText(/no classifier ran/)).not.toBeInTheDocument();
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
  vi.spyOn(globalThis, "fetch").mockImplementation((input) => {
    const url = typeof input === "string" ? input : (input as Request).url ?? String(input);
    // Engine health is independent of adoption, so the backend answers it even here (REQ051).
    if (url.endsWith("/api/engines")) return Promise.resolve(json({ engines: ENGINES }));
    return Promise.resolve(
      json({ error: "no companion tree found — run `provreq init` first" }, 409),
    );
  });
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
    classified_by: "classified",
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

// Verifies: REQ050 — a verdict with no recorded environment must not read like one whose
// environment was checked and found unchanged. Both are `fresh`, so if the surface said nothing
// the operator would infer a guarantee the record does not carry.
test("a stored verdict says where it was proved, or that it was never recorded", async () => {
  const user = userEvent.setup();
  const base: Detail = {
    id: "REQ001",
    title: "Login invariant",
    text: "A logged-in user always has a session.",
    revision: "r1",
    stale: false,
    classification: "formalizable-now",
    classified_by: "classified",
    formalization: "admitted",
    admission: { review: "optional", by: "gg" },
    candidate: "requirement r { category: 1 ... }",
    gate: { status: "passed", warnings: [] },
    readback: "At every state, if the user is logged in then the user has a session.",
    bindings: [],
    grounding: null,
    verdict: {
      status: "holds",
      basis: "proven",
      reason: null,
      detail: [],
      witness: null,
      evidence: [],
      fresh: true,
      stale_reasons: [],
      environment: "declared `lab-2`; Kani 0.67.0",
    },
  };

  mockRoutes(SAMPLE, { REQ001: base });
  const { unmount } = render(<App />);
  await user.click(await screen.findByRole("button", { name: "REQ001" }));
  let dialog = await screen.findByRole("dialog");
  expect(within(dialog).getByText(/Proved in:/)).toBeInTheDocument();
  expect(within(dialog).getByText(/declared `lab-2`; Kani 0\.67\.0/)).toBeInTheDocument();
  unmount();

  // The same fresh verdict, but from before environment recording existed.
  mockRoutes(SAMPLE, {
    REQ001: { ...base, verdict: { ...base.verdict!, environment: null } },
  });
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "REQ001" }));
  dialog = await screen.findByRole("dialog");
  expect(within(dialog).getByText(/Environment not recorded/)).toBeInTheDocument();
  expect(within(dialog).queryByText(/Proved in:/)).not.toBeInTheDocument();
});

// #218 — a stored verdict must show what it was reached on, not just what it concluded. The model a
// category-2a verdict was checked under and the counterexample behind a refutation live on the
// record; before this they rendered only for the operator who happened to press Verify that session.
test("a stored verdict shows its grounds — the model, the witness, and each engine", async () => {
  const user = userEvent.setup();
  const detail: Detail = {
    id: "REQ001",
    title: "Drone clearance",
    text: "A drone is never airborne unless cleared.",
    revision: "r1",
    stale: false,
    classification: "formalizable-now",
    classified_by: "classified",
    formalization: "admitted",
    admission: { review: "optional", by: "gg" },
    candidate: "requirement r { category: 2a ... }",
    gate: { status: "passed", warnings: [] },
    readback: "At every state, a drone that is airborne has been cleared.",
    bindings: [],
    grounding: null,
    verdict: {
      status: "fails",
      basis: "model-checked (bounded)",
      reason: null,
      detail: ["checked under the model — Drones = {d1, d2}, MaxAlt = 2"],
      witness: "state 2: alt = [d1 |-> 1], cleared = [d1 |-> FALSE]",
      evidence: [
        {
          engine: "TLC (TLA+)",
          status: "fails",
          basis: "model-checked (bounded)",
          witness: null,
          detail: ["checked under the model — Drones = {d1, d2}, MaxAlt = 2"],
        },
      ],
      fresh: true,
      stale_reasons: [],
      environment: "TLC (TLA+) 2.19",
    },
  };

  mockRoutes(SAMPLE, { REQ001: detail });
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "REQ001" }));
  const dialog = await screen.findByRole("dialog");

  // No Verify was pressed: everything below comes from the stored record alone.
  expect(
    within(dialog).getAllByText(/checked under the model — Drones = \{d1, d2\}, MaxAlt = 2/).length,
  ).toBeGreaterThan(0);
  expect(within(dialog).getByText(/state 2: alt = \[d1 \|-> 1\]/)).toBeInTheDocument();
  expect(within(dialog).getByText("TLC (TLA+)")).toBeInTheDocument();
});

test("an engine that cannot start reads as a fault, not as one that is merely absent (REQ051)", async () => {
  mockRoutes(SAMPLE, {}, [
    { category: "1", name: "Kani", state: "available", detail: "available (0.67.0)", reason: null },
    {
      category: "1",
      name: "Prusti",
      state: "unusable",
      detail: "PRESENT BUT UNUSABLE (error while loading shared libraries: libstd.so)",
      reason: "error while loading shared libraries: libstd.so",
    },
    { category: "2b", name: "MonPoly", state: "not-wired", detail: "NOT WIRED (no integration yet)", reason: null },
  ]);
  render(<App />);

  const engines = await screen.findByRole("region", { name: "Verification engines" });
  expect(within(engines).getByText("available")).toBeInTheDocument();
  expect(within(engines).getByText("cannot start")).toBeInTheDocument();
  // The whole point of the distinction: it must not read as "install it".
  expect(within(engines).queryByText("not installed")).not.toBeInTheDocument();
  const fault = within(engines).getByRole("status");
  expect(fault).toHaveTextContent(/Prusti is installed but cannot start/);
  expect(fault).toHaveTextContent(/libstd\.so/);
  expect(fault).toHaveTextContent(/Installing these again will not help/);
});

test("a healthy engine set shows no fault callout (REQ051)", async () => {
  mockRoutes(SAMPLE);
  render(<App />);

  const engines = await screen.findByRole("region", { name: "Verification engines" });
  expect(within(engines).getByText("Kani")).toBeInTheDocument();
  expect(within(engines).queryByRole("status")).not.toBeInTheDocument();
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
    if (url.endsWith("/api/engines")) return Promise.resolve(json({ engines: ENGINES }));
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
    classified_by: "classified",
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
    if (url.endsWith("/api/engines")) return Promise.resolve(json({ engines: ENGINES }));
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

test("'Re-verify all stale' re-runs each drifted item then refreshes the funnel (REQ044)", async () => {
  const user = userEvent.setup();
  // The settled backlog the refresh returns: REQ003's verdict is fresh again, stale count is 0.
  const AFTER: Backlog = {
    ...SAMPLE,
    coverage: { ...SAMPLE.coverage, stale: 0, verified: 1 },
    items: SAMPLE.items.map((i) =>
      i.id === "REQ003" && i.verdict
        ? { ...i, verdict: { ...i.verdict, fresh: true, stale_reasons: [] } }
        : i,
    ),
  };
  const verified: string[] = [];
  let listCalls = 0;
  vi.spyOn(globalThis, "fetch").mockImplementation((input, init) => {
    const url = typeof input === "string" ? input : (input as Request).url ?? String(input);
    const method = (init?.method ?? "GET").toUpperCase();
    if (url.endsWith("/api/engines")) return Promise.resolve(json({ engines: ENGINES }));
    if (url.endsWith("/verify") && method === "POST") {
      const id = url.match(/requirements\/(.+)\/verify$/)![1];
      verified.push(decodeURIComponent(id));
      return Promise.resolve(json({ state: "verdict", stale: false, verdict: {} }));
    }
    // The list GET: stale before the sweep, settled after it.
    listCalls += 1;
    return Promise.resolve(json(listCalls === 1 ? SAMPLE : AFTER));
  });
  render(<App />);

  await user.click(await screen.findByRole("button", { name: /re-verify all stale \(1\)/i }));

  // Only REQ003 — the sole drifted item — is re-run; REQ001's fresh holds is left alone.
  await waitFor(() => expect(verified).toEqual(["REQ003"]));
  // After the refresh the funnel settled, so the action is gone.
  await waitFor(() =>
    expect(
      screen.queryByRole("button", { name: /re-verify all stale/i }),
    ).not.toBeInTheDocument(),
  );
});

test("a failed triage write rolls back and surfaces an error", async () => {
  const user = userEvent.setup();
  vi.spyOn(globalThis, "fetch").mockImplementation((input) => {
    const url = typeof input === "string" ? input : (input as Request).url;
    if (url.endsWith("/api/engines")) return Promise.resolve(json({ engines: ENGINES }));
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
