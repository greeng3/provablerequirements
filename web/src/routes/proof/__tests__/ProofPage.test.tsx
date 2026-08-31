import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TestQueryProvider } from "../../../test-utils";
import type {
  ProofBacklog,
  ProofDetail,
  ProofEngineReport,
  ProofVerifyResponse,
} from "../../../api/types";
import { ProofPage } from "../ProofPage";

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

const ENGINES: ProofEngineReport[] = [
  {
    category: "1",
    name: "Kani",
    state: "available",
    detail: "available (0.67.0)",
    reason: null,
  },
  {
    category: "2a",
    name: "TLC (TLA+)",
    state: "available",
    detail: "available (2.19)",
    reason: null,
  },
];

const SAMPLE: ProofBacklog = {
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
    {
      id: "REQ001",
      title: "Login invariant",
      text: "prose",
      classification: "formalizable-now",
      classified_by: "classified",
      formalization: "drafting",
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
    },
    {
      id: "REQ002",
      title: null,
      text: "some prose here",
      classification: null,
      classified_by: null,
      formalization: "none",
      verdict: null,
    },
    {
      id: "REQ003",
      title: "A note",
      text: "prose",
      classification: "stays-prose",
      classified_by: "seeded",
      formalization: "none",
      verdict: {
        status: "holds",
        basis: "proven",
        reason: null,
        detail: [],
        witness: null,
        evidence: [],
        fresh: false,
        stale_reasons: ["the subject code moved — re-verify"],
        environment: null,
      },
    },
  ],
};

type MockOptions = {
  backlog?: ProofBacklog;
  details?: Record<string, ProofDetail>;
  engines?: ProofEngineReport[];
  triageResult?: ProofBacklog;
  verifyResult?: ProofVerifyResponse;
  onTriage?: (id: string) => void;
  onVerify?: (id: string) => void;
};

/// Route fetch by URL + method: the engine probe, the list, a per-id detail,
/// the triage POST, and the verify POST. Mirrors the shape the rest of the
/// frontend's tests mock (stubGlobal fetch + typed Response bodies).
function mockRoutes(opts: MockOptions = {}) {
  const backlog = opts.backlog ?? SAMPLE;
  const details = opts.details ?? {};
  const engines = opts.engines ?? ENGINES;
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const stripped = url.split("?")[0] ?? url;
    const method = (init?.method ?? "GET").toUpperCase();

    if (stripped.endsWith("/api/engines")) return json({ engines });

    const triageMatch = stripped.match(/\/api\/requirements\/(.+)\/triage$/);
    if (triageMatch && method === "POST") {
      opts.onTriage?.(decodeURIComponent(triageMatch[1]));
      return json(opts.triageResult ?? backlog);
    }

    const verifyMatch = stripped.match(/\/api\/requirements\/(.+)\/verify$/);
    if (verifyMatch && method === "POST") {
      opts.onVerify?.(decodeURIComponent(verifyMatch[1]));
      return json(
        opts.verifyResult ?? { state: "no-draft" },
      );
    }

    const detailMatch = stripped.match(/\/api\/requirements\/([^/]+)$/);
    if (detailMatch) {
      const d = details[decodeURIComponent(detailMatch[1])];
      return d ? json(d) : json({ error: "not found" }, 404);
    }

    if (stripped.endsWith("/api/requirements")) return json(backlog);

    return json({}, 200);
  });
}

function renderPage() {
  return render(
    <TestQueryProvider>
      <ProofPage />
    </TestQueryProvider>,
  );
}

describe("ProofPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders the backlog, coverage funnel, and engine health", async () => {
    mockRoutes();
    renderPage();

    expect(await screen.findByText("REQ001")).toBeInTheDocument();
    expect(screen.getByText("REQ002")).toBeInTheDocument();
    expect(screen.getByText("REQ003")).toBeInTheDocument();
    expect(screen.getByText("3 discovered")).toBeInTheDocument();

    const engines = await screen.findByRole("region", {
      name: "Verification engines",
    });
    expect(within(engines).getByText("Kani")).toBeInTheDocument();
    // A healthy set shows no fault callout.
    expect(within(engines).queryByRole("status")).not.toBeInTheDocument();
  });

  it("annotates a seeded classification but not a judged one (#180)", async () => {
    mockRoutes();
    renderPage();
    await screen.findByText("REQ001");

    const table = screen.getByRole("table");
    const seeded = within(table).getByText("REQ003").closest("tr") as HTMLElement;
    expect(
      within(seeded).getByText("seeded — no classifier ran"),
    ).toBeInTheDocument();
    const judged = within(table).getByText("REQ001").closest("tr") as HTMLElement;
    expect(
      within(judged).queryByText(/no classifier ran/),
    ).not.toBeInTheDocument();
  });

  it("filter tabs narrow the list", async () => {
    const user = userEvent.setup();
    mockRoutes();
    renderPage();
    await screen.findByText("REQ001");

    await user.click(screen.getByRole("tab", { name: "Untriaged" }));

    const table = screen.getByRole("table");
    expect(within(table).getByText("REQ002")).toBeInTheDocument();
    expect(within(table).queryByText("REQ001")).not.toBeInTheDocument();
    expect(within(table).queryByText("REQ003")).not.toBeInTheDocument();
  });

  it("marks a drifted stored verdict stale in the backlog (REQ039)", async () => {
    mockRoutes();
    renderPage();
    await screen.findByText("REQ001");

    const table = screen.getByRole("table");
    const stale = within(table).getByText("⟳ stale");
    expect(stale).toHaveAttribute(
      "title",
      expect.stringContaining("subject code moved"),
    );
    expect(within(table).getByText("not verified")).toBeInTheDocument();
  });

  it("writing a row's triage bucket POSTs and reflects the new bucket", async () => {
    const user = userEvent.setup();
    const triaged: string[] = [];
    const after: ProofBacklog = {
      coverage: { ...SAMPLE.coverage, untriaged: 0, stays_prose: 2 },
      items: SAMPLE.items.map((i) =>
        i.id === "REQ002" ? { ...i, classification: "stays-prose" } : i,
      ),
    };
    mockRoutes({ triageResult: after, onTriage: (id) => triaged.push(id) });
    renderPage();
    await screen.findByText("REQ001");

    const select = screen.getByLabelText(
      "Triage bucket for REQ002",
    ) as HTMLSelectElement;
    await user.selectOptions(select, "stays-prose");

    await waitFor(() => expect(select.value).toBe("stays-prose"));
    expect(triaged).toEqual(["REQ002"]);
  });

  it("a failed triage write rolls back and surfaces an error", async () => {
    const user = userEvent.setup();
    vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const stripped = url.split("?")[0] ?? url;
      const method = (init?.method ?? "GET").toUpperCase();
      if (stripped.endsWith("/api/engines")) return json({ engines: ENGINES });
      if (stripped.endsWith("/triage") && method === "POST")
        return json({ error: "disk full" }, 409);
      if (stripped.endsWith("/api/requirements")) return json(SAMPLE);
      return json({}, 200);
    });
    renderPage();
    await screen.findByText("REQ001");

    const select = screen.getByLabelText(
      "Triage bucket for REQ002",
    ) as HTMLSelectElement;
    await user.selectOptions(select, "stays-prose");

    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    await waitFor(() => expect(select.value).toBe(""));
  });

  it("opens a requirement's detail dialog with its candidate and read-back", async () => {
    const user = userEvent.setup();
    const detail: ProofDetail = {
      id: "REQ001",
      title: "Login invariant",
      text: "A logged-in user always has a session.",
      revision: "r1",
      stale: false,
      classification: "formalizable-now",
      classified_by: "classified",
      formalization: "admitted",
      admission: { review: "optional", by: "gg" },
      candidate: "requirement r { category: 1 }",
      gate: { status: "passed", warnings: [] },
      readback: "If the user is logged in then the user has a session.",
      bindings: [],
      grounding: {
        grounded: true,
        bindings: [
          {
            symbol: "logged_in",
            observable: "login",
            category: "code",
            resolved: true,
            summary: "logged_in → `login` resolves to src/lib.rs:1",
          },
        ],
      },
      verdict: null,
    };
    mockRoutes({ details: { REQ001: detail } });
    renderPage();

    await user.click(await screen.findByRole("button", { name: "REQ001" }));

    const dialog = await screen.findByRole("dialog");
    expect(
      await within(dialog).findByText(/if the user is logged in/i),
    ).toBeInTheDocument();
    expect(within(dialog).getByText(/requirement r \{/)).toBeInTheDocument();
    expect(within(dialog).getByText("grounded")).toBeInTheDocument();
    expect(
      within(dialog).getByText(/resolves to src\/lib\.rs:1/),
    ).toBeInTheDocument();
  });

  it("running Verify shows the verdict with per-engine evidence", async () => {
    const user = userEvent.setup();
    const detail: ProofDetail = {
      id: "REQ001",
      title: "Login invariant",
      text: "A logged-in user always has a session.",
      revision: "r1",
      stale: false,
      classification: "formalizable-now",
      classified_by: "classified",
      formalization: "admitted",
      admission: { review: "optional", by: "gg" },
      candidate: "requirement r { category: 1 }",
      gate: { status: "passed", warnings: [] },
      readback: "At every state…",
      bindings: [],
      grounding: null,
      verdict: null,
    };
    const verdict: ProofVerifyResponse = {
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
          {
            engine: "Creusot",
            status: "holds",
            basis: "proven",
            witness: null,
            detail: [],
          },
          {
            engine: "Kani",
            status: "unknown",
            basis: null,
            witness: null,
            detail: ["harness would not compile"],
          },
        ],
        provenance: {
          requirement_revision: "r1",
          subject_commit: "abc123",
          tool_version: "0.0.8",
        },
      },
    };
    const verified: string[] = [];
    mockRoutes({
      details: { REQ001: detail },
      verifyResult: verdict,
      onVerify: (id) => verified.push(id),
    });
    renderPage();

    await user.click(await screen.findByRole("button", { name: "REQ001" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Verify" }));

    // Aggregate polarity + Creusot both "holds".
    expect(await within(dialog).findAllByText("holds")).toHaveLength(2);
    expect(within(dialog).getByText("Creusot")).toBeInTheDocument();
    expect(within(dialog).getByText("Kani")).toBeInTheDocument();
    expect(
      within(dialog).getByText("harness would not compile"),
    ).toBeInTheDocument();
    expect(verified).toEqual(["REQ001"]);
  });

  it("an engine that cannot start reads as a fault, not as merely absent (REQ051)", async () => {
    mockRoutes({
      engines: [
        {
          category: "1",
          name: "Kani",
          state: "available",
          detail: "available (0.67.0)",
          reason: null,
        },
        {
          category: "1",
          name: "Prusti",
          state: "unusable",
          detail: "PRESENT BUT UNUSABLE (libstd.so)",
          reason: "error while loading shared libraries: libstd.so",
        },
      ],
    });
    renderPage();

    const engines = await screen.findByRole("region", {
      name: "Verification engines",
    });
    expect(within(engines).getByText("cannot start")).toBeInTheDocument();
    expect(within(engines).queryByText("not installed")).not.toBeInTheDocument();
    const fault = within(engines).getByRole("status");
    expect(fault).toHaveTextContent(/Prusti is installed but cannot start/);
    expect(fault).toHaveTextContent(/libstd\.so/);
    expect(fault).toHaveTextContent(/Installing these again will not help/);
  });

  it("surfaces the backend error when the subject is not adopted", async () => {
    vi.stubGlobal("fetch", async (input: RequestInfo) => {
      const url = typeof input === "string" ? input : input.toString();
      const stripped = url.split("?")[0] ?? url;
      // Engine health is independent of adoption (REQ051).
      if (stripped.endsWith("/api/engines")) return json({ engines: ENGINES });
      return json(
        { error: "no companion tree found — run `provreq init` first" },
        409,
      );
    });
    renderPage();

    expect(await screen.findByRole("alert")).toHaveTextContent("provreq init");
  });

  it("'Re-verify all stale' re-runs each drifted item then refreshes", async () => {
    const user = userEvent.setup();
    const AFTER: ProofBacklog = {
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
    vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const stripped = url.split("?")[0] ?? url;
      const method = (init?.method ?? "GET").toUpperCase();
      if (stripped.endsWith("/api/engines")) return json({ engines: ENGINES });
      const verifyMatch = stripped.match(/\/api\/requirements\/(.+)\/verify$/);
      if (verifyMatch && method === "POST") {
        verified.push(decodeURIComponent(verifyMatch[1]));
        return json({ state: "verdict", stale: false, verdict: {} });
      }
      if (stripped.endsWith("/api/requirements")) {
        listCalls += 1;
        return json(listCalls === 1 ? SAMPLE : AFTER);
      }
      return json({}, 200);
    });
    renderPage();

    await user.click(
      await screen.findByRole("button", { name: /re-verify all stale \(1\)/i }),
    );

    await waitFor(() => expect(verified).toEqual(["REQ003"]));
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: /re-verify all stale/i }),
      ).not.toBeInTheDocument(),
    );
  });
});
