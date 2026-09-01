import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TestQueryProvider } from "../../../../test-utils";
import type { ProofVerdictView } from "../../../../api/types";
import { VerifyPanel } from "../VerifyPanel";

// A stored verdict with everything the panel needs; the fields under test are
// `environment` and `fresh` — the rest is filler so StoredVerdict renders.
function storedVerdict(
  overrides: Partial<ProofVerdictView> = {},
): ProofVerdictView {
  return {
    status: "holds",
    basis: "proven",
    reason: null,
    detail: [],
    witness: null,
    evidence: [],
    fresh: true,
    stale_reasons: [],
    environment: "declared `lab-2`; Kani 0.67.0",
    ...overrides,
  };
}

function renderPanel(stored: ProofVerdictView | null) {
  render(
    <TestQueryProvider>
      <VerifyPanel id="REQ050" stored={stored} />
    </TestQueryProvider>,
  );
}

describe("VerifyPanel stored-verdict provenance", () => {
  // Verifies: REQ050 — a stored verdict says where it was proved, or that it was never recorded
  it("names the environment a recorded verdict was proved in", () => {
    renderPanel(storedVerdict({ environment: "declared `lab-2`; Kani 0.67.0" }));

    expect(screen.getByText("Proved in:")).toBeInTheDocument();
    expect(
      screen.getByText(/declared `lab-2`; Kani 0\.67\.0/),
    ).toBeInTheDocument();
  });

  // Verifies: REQ050 — an unrecorded environment is called out, not left to read as unchanged
  it("calls out a verdict whose environment was never recorded", () => {
    renderPanel(storedVerdict({ environment: null }));

    expect(screen.getByText("Environment not recorded")).toBeInTheDocument();
    expect(screen.queryByText("Proved in:")).not.toBeInTheDocument();
  });
});
