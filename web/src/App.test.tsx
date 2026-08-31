import { render, screen } from "@testing-library/react";
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

  it("renders the System Home route by default", () => {
    stubFetch();
    render(<App />);
    expect(
      screen.getByRole("heading", { name: /System Home/i }),
    ).toBeInTheDocument();
  });
});
