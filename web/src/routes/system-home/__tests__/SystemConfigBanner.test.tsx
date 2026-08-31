import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { SystemConfigBanner } from "../SystemConfigBanner";

interface HandlerResult {
  status: number;
  body: unknown;
}

function stubFetch(handler: (url: string) => HandlerResult) {
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
        <SystemConfigBanner />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("SystemConfigBanner", () => {
  beforeEach(() => {
    sessionStorage.clear();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    sessionStorage.clear();
  });

  it("renders when projectCount >= 2 and no system is loaded", async () => {
    stubFetch(() => ({
      status: 200,
      body: { loaded: false, projectCount: 3 },
    }));
    mount();
    const banner = await screen.findByTestId("system-config-banner");
    expect(banner).toHaveTextContent("3 projects are mounted");
    expect(banner).toHaveTextContent("REQFORGE_SYSTEM_CONFIG");
  });

  it("stays hidden when a system config is loaded", async () => {
    stubFetch(() => ({
      status: 200,
      body: { loaded: true, name: "My System", projectCount: 3 },
    }));
    mount();
    await waitFor(
      () => {
        expect(screen.queryByTestId("system-config-banner")).toBeNull();
      },
      { timeout: 1000 },
    );
  });

  it("stays hidden when only one project is mounted", async () => {
    stubFetch(() => ({
      status: 200,
      body: { loaded: false, projectCount: 1 },
    }));
    mount();
    await waitFor(
      () => {
        expect(screen.queryByTestId("system-config-banner")).toBeNull();
      },
      { timeout: 1000 },
    );
  });

  it("can be dismissed and persists the dismissal for the session", async () => {
    stubFetch(() => ({
      status: 200,
      body: { loaded: false, projectCount: 2 },
    }));
    const { unmount } = mount();
    const banner = await screen.findByTestId("system-config-banner");
    await userEvent.click(screen.getByTestId("system-config-banner-dismiss"));
    expect(banner).not.toBeInTheDocument();
    // sessionStorage carries the decision so a fresh mount
    // (simulating re-render / navigation) stays quiet.
    unmount();
    mount();
    await waitFor(
      () => {
        expect(screen.queryByTestId("system-config-banner")).toBeNull();
      },
      { timeout: 1000 },
    );
  });
});
