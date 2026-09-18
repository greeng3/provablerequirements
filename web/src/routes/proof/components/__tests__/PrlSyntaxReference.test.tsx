import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { PrlSyntaxReference } from "../PrlSyntaxReference";

describe("PrlSyntaxReference", () => {
  // Verifies: #465 — the drafting page carries an in-product PRL syntax
  // reference, so a gate failure has something to consult without leaving.
  it("is collapsed by default and keyboard-reachable", () => {
    render(<PrlSyntaxReference />);

    // A native <summary> is focusable, so the disclosure is keyboard-operable
    // without any custom key handling; its <details> parent starts closed.
    const summary = screen.getByText(/prl syntax/i);
    expect(summary.tagName).toBe("SUMMARY");
    expect(summary.closest("details")).not.toHaveAttribute("open");
  });

  it("states the one-pattern-per-property rule the pilot got stuck on", async () => {
    const user = userEvent.setup();
    render(<PrlSyntaxReference />);

    await user.click(screen.getByText(/prl syntax/i));

    expect(
      screen.getByText(/exactly one pattern per property/i),
    ).toBeInTheDocument();
  });
});
