import { render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";

function stubFetch() {
  vi.stubGlobal(
    "fetch",
    async () =>
      new Response(JSON.stringify([]), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("App landing page", () => {
  it("renders the ReqForge brand in the header", () => {
    stubFetch();
    render(<App />);
    const brand = screen.getByRole("link", { name: /ReqForge/i });
    expect(brand).toBeInTheDocument();
    expect(brand).toHaveAttribute("href", "/");
  });

  it("shows a no-project message at / when none is served", async () => {
    stubFetch();
    render(<App />);
    // Scope to the main content — the sidebar renders its own
    // "No project found." copy for the empty subject.
    const main = within(screen.getByRole("main"));
    expect(await main.findByText(/no project found/i)).toBeInTheDocument();
  });
});
