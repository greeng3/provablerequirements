import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { WipeArtifactsDialog } from "../WipeArtifactsDialog";

interface HandlerResult {
  status: number;
  body: unknown;
}

function stubFetch(handler: (url: string, method: string) => HandlerResult) {
  const calls: Array<{ url: string; method: string }> = [];
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const method = init?.method ?? "GET";
    calls.push({ url, method });
    const result = handler(url, method);
    return new Response(
      result.body === undefined ? null : JSON.stringify(result.body),
      {
        status: result.status,
        headers: { "content-type": "application/json" },
      },
    );
  });
  return calls;
}

function mount(onClose = () => {}) {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <WipeArtifactsDialog projectSlug="reqforge" onClose={onClose} />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("WipeArtifactsDialog", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("disables the wipe button until the slug is typed exactly", async () => {
    mount();
    const button = screen.getByTestId("wipe-artifacts-confirm");
    expect(button).toBeDisabled();

    const input = screen.getByTestId("wipe-artifacts-confirm-input");
    await userEvent.type(input, "wrong");
    expect(button).toBeDisabled();

    await userEvent.clear(input);
    await userEvent.type(input, "reqforge");
    expect(button).toBeEnabled();
  });

  it("DELETEs without ?deinit when the checkbox is unchecked", async () => {
    const calls = stubFetch((url, method) => {
      if (
        method === "DELETE" &&
        url.includes("/api/projects/reqforge/artifacts")
      ) {
        return { status: 204, body: undefined };
      }
      return { status: 404, body: { error: "unexpected" } };
    });
    const onClose = vi.fn();
    mount(onClose);

    await userEvent.type(
      screen.getByTestId("wipe-artifacts-confirm-input"),
      "reqforge",
    );
    await userEvent.click(screen.getByTestId("wipe-artifacts-confirm"));

    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    const deleteCall = calls.find((c) => c.method === "DELETE");
    expect(deleteCall).toBeDefined();
    expect(deleteCall!.url).toContain("/api/projects/reqforge/artifacts");
    expect(deleteCall!.url).not.toContain("deinit=true");
  });

  it("DELETEs with ?deinit=true and adapts the heading + button label when the checkbox is checked", async () => {
    const calls = stubFetch((url, method) => {
      if (
        method === "DELETE" &&
        url.includes("/api/projects/reqforge/artifacts")
      ) {
        return { status: 204, body: undefined };
      }
      return { status: 404, body: { error: "unexpected" } };
    });
    const onClose = vi.fn();
    mount(onClose);

    // Default heading + button label.
    expect(
      screen.getByRole("heading", { name: /wipe all artifacts/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^wipe artifacts$/i }),
    ).toBeInTheDocument();

    // Toggle checkbox — heading + label flip, deinit note shows.
    await userEvent.click(screen.getByTestId("wipe-artifacts-deinit-checkbox"));
    expect(
      screen.getByRole("heading", { name: /wipe and de-initialize/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^wipe and de-initialize$/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("wipe-artifacts-deinit-note"),
    ).toBeInTheDocument();

    await userEvent.type(
      screen.getByTestId("wipe-artifacts-confirm-input"),
      "reqforge",
    );
    await userEvent.click(screen.getByTestId("wipe-artifacts-confirm"));

    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    const deleteCall = calls.find((c) => c.method === "DELETE");
    expect(deleteCall).toBeDefined();
    expect(deleteCall!.url).toContain(
      "/api/projects/reqforge/artifacts?deinit=true",
    );
  });

  it("surfaces a server error and keeps the dialog open", async () => {
    stubFetch((url, method) => {
      if (
        method === "DELETE" &&
        url.endsWith("/api/projects/reqforge/artifacts")
      ) {
        return { status: 500, body: { error: "wipe collection dirs: nope" } };
      }
      return { status: 404, body: {} };
    });
    const onClose = vi.fn();
    mount(onClose);

    await userEvent.type(
      screen.getByTestId("wipe-artifacts-confirm-input"),
      "reqforge",
    );
    await userEvent.click(screen.getByTestId("wipe-artifacts-confirm"));

    await waitFor(() =>
      expect(screen.getByTestId("wipe-artifacts-error")).toBeInTheDocument(),
    );
    expect(onClose).not.toHaveBeenCalled();
  });
});
