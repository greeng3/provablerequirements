import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { KeyboardShortcutsOverlay } from "../KeyboardShortcutsOverlay";

describe("KeyboardShortcutsOverlay", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <KeyboardShortcutsOverlay open={false} onClose={() => {}} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders the shortcut groups when open", () => {
    render(<KeyboardShortcutsOverlay open={true} onClose={() => {}} />);
    expect(
      screen.getByTestId("keyboard-shortcuts-overlay"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: /Keyboard shortcuts/i }),
    ).toBeInTheDocument();
    // The overlay lists shortcut groups + individual bindings.
    expect(screen.getByText(/Open this help/i)).toBeInTheDocument();
    expect(screen.getByText(/Close any open dialog/i)).toBeInTheDocument();
    expect(screen.getByText(/Save the artifact/i)).toBeInTheDocument();
  });

  it("closes on Escape", async () => {
    const onClose = vi.fn();
    render(<KeyboardShortcutsOverlay open={true} onClose={onClose} />);
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });

  it("closes when the backdrop is clicked", async () => {
    const onClose = vi.fn();
    render(<KeyboardShortcutsOverlay open={true} onClose={onClose} />);
    await userEvent.click(screen.getByTestId("keyboard-shortcuts-overlay"));
    expect(onClose).toHaveBeenCalled();
  });

  it("does not close when clicking inside the dialog body", async () => {
    const onClose = vi.fn();
    render(<KeyboardShortcutsOverlay open={true} onClose={onClose} />);
    await userEvent.click(
      screen.getByRole("heading", { name: /Keyboard shortcuts/i }),
    );
    expect(onClose).not.toHaveBeenCalled();
  });
});
