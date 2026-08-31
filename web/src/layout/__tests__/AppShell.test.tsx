import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";

import { TestQueryProvider } from "../../test-utils";
import { AppShell } from "../AppShell";

interface HandlerResult {
  status: number;
  body: unknown;
}

function stubFetch(
  handler: (url: string) => HandlerResult = () => ({
    status: 200,
    body: [],
  }),
) {
  vi.stubGlobal("fetch", async (input: RequestInfo) => {
    const url = typeof input === "string" ? input : input.toString();
    const result = handler(url);
    return new Response(JSON.stringify(result.body), {
      status: result.status,
      headers: { "content-type": "application/json" },
    });
  });
}

function mount() {
  return render(
    <TestQueryProvider>
      <MemoryRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<p data-testid="home-body">home</p>} />
          </Route>
        </Routes>
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("AppShell", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("exposes a skip-to-main link pointing at the main region", () => {
    stubFetch();
    mount();
    const skipLink = screen.getByTestId("skip-to-main");
    expect(skipLink).toHaveAttribute("href", "#main");
    // The target element has the id the skip link points at.
    expect(document.getElementById("main")).toBeInTheDocument();
  });

  it("mounts the header keyboard-shortcuts button", () => {
    stubFetch();
    mount();
    expect(screen.getByTestId("header-shortcuts-button")).toBeInTheDocument();
  });

  it("opens the keyboard-shortcuts overlay on the global ? hotkey", async () => {
    stubFetch();
    mount();
    expect(screen.queryByTestId("keyboard-shortcuts-overlay")).toBeNull();
    await userEvent.keyboard("?");
    await waitFor(() =>
      expect(
        screen.getByTestId("keyboard-shortcuts-overlay"),
      ).toBeInTheDocument(),
    );
  });

  it("suppresses the ? hotkey while an input has focus", async () => {
    stubFetch();
    render(
      <TestQueryProvider>
        <MemoryRouter>
          <Routes>
            <Route element={<AppShell />}>
              <Route
                path="/"
                element={
                  <input data-testid="focused-input" aria-label="focus trap" />
                }
              />
            </Route>
          </Routes>
        </MemoryRouter>
      </TestQueryProvider>,
    );
    const input = screen.getByTestId("focused-input");
    input.focus();
    await userEvent.keyboard("?");
    expect(screen.queryByTestId("keyboard-shortcuts-overlay")).toBeNull();
  });

  it("opens the overlay when the header button is clicked", async () => {
    stubFetch();
    mount();
    await userEvent.click(screen.getByTestId("header-shortcuts-button"));
    expect(
      screen.getByTestId("keyboard-shortcuts-overlay"),
    ).toBeInTheDocument();
  });
});
