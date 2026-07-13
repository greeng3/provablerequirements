import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { App } from "./App";

afterEach(() => {
  vi.restoreAllMocks();
});

test("renders the health status and version from /health", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(JSON.stringify({ status: "ok", version: "9.9.9" }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    }),
  );

  render(<App />);

  expect(await screen.findByText("ok")).toBeInTheDocument();
  expect(screen.getByText("9.9.9")).toBeInTheDocument();
});

test("shows an error when the backend is unreachable", async () => {
  vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("boom"));

  render(<App />);

  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("boom"));
});
