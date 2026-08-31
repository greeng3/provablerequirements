import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { BulkUrlCheckButton } from "../BulkUrlCheckButton";

describe("BulkUrlCheckButton", () => {
  it("labels itself with the URL-artifact count", () => {
    render(
      <BulkUrlCheckButton
        urlArtifactCount={3}
        pending={false}
        error={null}
        onCheck={() => {}}
      />,
    );
    expect(
      screen.getByRole("button", { name: /check 3 urls/i }),
    ).toBeInTheDocument();
  });

  it("uses the singular form when there is exactly one URL artifact", () => {
    render(
      <BulkUrlCheckButton
        urlArtifactCount={1}
        pending={false}
        error={null}
        onCheck={() => {}}
      />,
    );
    expect(
      screen.getByRole("button", { name: /check 1 url$/i }),
    ).toBeInTheDocument();
  });

  it("shows a 'Checking…' label while pending and disables the button", () => {
    render(
      <BulkUrlCheckButton
        urlArtifactCount={2}
        pending={true}
        error={null}
        onCheck={() => {}}
      />,
    );
    const button = screen.getByRole("button", { name: /checking/i });
    expect(button).toBeDisabled();
  });

  it("invokes onCheck when clicked", async () => {
    const onCheck = vi.fn();
    render(
      <BulkUrlCheckButton
        urlArtifactCount={2}
        pending={false}
        error={null}
        onCheck={onCheck}
      />,
    );
    await userEvent.click(
      screen.getByRole("button", { name: /check 2 urls/i }),
    );
    expect(onCheck).toHaveBeenCalledOnce();
  });

  it("renders the pass/fail summary once a result is present", () => {
    render(
      <BulkUrlCheckButton
        urlArtifactCount={3}
        pending={false}
        error={null}
        onCheck={() => {}}
        result={{
          checked: [
            {
              uuid: "u1",
              checkedAt: "2026-04-20T12:00:00Z",
              checkStatus: "ok",
            },
            {
              uuid: "u2",
              checkedAt: "2026-04-20T12:00:00Z",
              checkStatus: "not-found",
            },
            {
              uuid: "u3",
              checkedAt: "2026-04-20T12:00:00Z",
              checkStatus: "ok",
            },
          ],
        }}
      />,
    );
    expect(
      screen.getByText((_, node) => node?.textContent === "2/3 OK · 1 failure"),
    ).toBeInTheDocument();
  });
});
