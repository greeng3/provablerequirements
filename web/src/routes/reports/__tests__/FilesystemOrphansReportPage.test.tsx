import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { TestQueryProvider } from "../../../test-utils";
import { FilesystemOrphansReportPage } from "../FilesystemOrphansReportPage";

function stubFetchByPath(responses: Record<string, unknown>) {
  const ordered = Object.keys(responses).sort((a, b) => b.length - a.length);
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString();
    const match = ordered.find((path) => {
      const stripped = url.split("?")[0] ?? url;
      return stripped.endsWith(path);
    });
    if (match === undefined) {
      return new Response(JSON.stringify({}), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    return new Response(JSON.stringify(responses[match]), {
      status: init?.method === "PUT" ? 204 : 200,
      headers: { "content-type": "application/json" },
    });
  });
}

function renderPage() {
  return render(
    <TestQueryProvider>
      <MemoryRouter initialEntries={["/reports/filesystem-orphans"]}>
        <FilesystemOrphansReportPage />
      </MemoryRouter>
    </TestQueryProvider>,
  );
}

describe("FilesystemOrphansReportPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("shows the empty-state copy when there are no orphans", async () => {
    stubFetchByPath({
      "/api/reports/filesystem-orphans": {
        kind: "filesystem-orphans",
        scope: { kind: "system" },
        missingSidecar: [],
        missingBinary: [],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByText(/no filesystem-orphans in scope/i),
      ).toBeInTheDocument(),
    );
  });

  it("renders both tables and exposes an Adopt-as-artifact button per binary", async () => {
    stubFetchByPath({
      "/api/reports/filesystem-orphans": {
        kind: "filesystem-orphans",
        scope: { kind: "system" },
        missingSidecar: [
          {
            projectSlug: "sample",
            collectionPrefix: "DES",
            filename: "DES-logo.png",
            binaryRelativePath: "artifacts/DES/DES-logo.png",
            byteSize: 1024,
            mediaType: "image/png",
          },
        ],
        missingBinary: [
          {
            projectSlug: "sample",
            collectionPrefix: "DES",
            sidecarFilename: "DES-ghost.pdf.reqforge.json",
            declaredBlobPath: "artifacts/DES/DES-ghost.pdf",
          },
        ],
      },
      "/api/projects": [],
    });
    renderPage();
    await waitFor(() =>
      expect(
        screen.getByText("artifacts/DES/DES-logo.png"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText("DES-ghost.pdf.reqforge.json")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /adopt as artifact/i }),
    ).toBeInTheDocument();
  });

  it("opens the Adopt dialog when the button is clicked", async () => {
    stubFetchByPath({
      "/api/reports/filesystem-orphans": {
        kind: "filesystem-orphans",
        scope: { kind: "system" },
        missingSidecar: [
          {
            projectSlug: "sample",
            collectionPrefix: "DES",
            filename: "DES-logo.png",
            binaryRelativePath: "artifacts/DES/DES-logo.png",
            byteSize: 1024,
            mediaType: "image/png",
          },
        ],
        missingBinary: [],
      },
      "/api/projects": [],
    });
    renderPage();
    const button = await screen.findByRole("button", {
      name: /adopt as artifact/i,
    });
    await userEvent.click(button);
    expect(
      screen.getByRole("dialog", { name: /adopt as artifact/i }),
    ).toBeInTheDocument();
  });
});
