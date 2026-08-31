import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TestQueryProvider } from "../../../../test-utils";
import type { MatrixNodeDto } from "../../../../api/types";
import { MatrixCellDialog } from "../MatrixCellDialog";

const row: MatrixNodeDto = {
  uuid: "r1",
  projectSlug: "sample",
  collectionPrefix: "REQ",
  artifactName: "REQ-a",
  title: "A",
  shape: "content",
  active: true,
  derived: false,
  tags: [],
  reviewState: "approved",
};

const column: MatrixNodeDto = {
  uuid: "c1",
  projectSlug: "sample",
  collectionPrefix: "DES",
  artifactName: "DES-a",
  title: "Design A",
  shape: "content",
  active: true,
  derived: false,
  tags: [],
  reviewState: "never-reviewed",
};

function artifactDetail(
  uuid: string,
  name: string,
  links: Array<{
    targetUuid: string;
    type: string;
    hint: {
      projectSlug: string;
      collectionPrefix: string;
      artifactName: string;
    };
  }>,
) {
  return {
    name,
    projectSlug: "sample",
    collectionPrefix: "REQ",
    uuid,
    title: "A",
    shape: "content",
    description: null,
    active: true,
    derived: false,
    createdAt: "2026-04-22T00:00:00Z",
    modifiedAt: "2026-04-22T00:00:00Z",
    tags: [],
    links: links.map((l) => ({
      ...l,
      resolution: { state: "resolved" },
    })),
    reviewLog: [],
    reviewState: { state: "none" },
    body: "",
  };
}

describe("MatrixCellDialog", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("creates a new link by PUTing existing links plus the new entry", async () => {
    const putBodies: unknown[] = [];
    vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const stripped = url.split("?")[0] ?? url;
      if (stripped.endsWith("/api/artifacts/r1") && init?.method === "PUT") {
        putBodies.push(init.body ? JSON.parse(String(init.body)) : undefined);
        return new Response("{}", {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      if (stripped.endsWith("/api/artifacts/r1")) {
        // REQ-a already has a verifies link to some other
        // artifact; create must preserve it.
        return new Response(
          JSON.stringify(
            artifactDetail("r1", "REQ-a", [
              {
                targetUuid: "other",
                type: "verifies",
                hint: {
                  projectSlug: "sample",
                  collectionPrefix: "REQ",
                  artifactName: "REQ-other",
                },
              },
            ]),
          ),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      return new Response("{}", {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    });
    const onToggled = vi.fn();
    const onClose = vi.fn();
    render(
      <TestQueryProvider>
        <MatrixCellDialog
          row={row}
          column={column}
          linkType="satisfies"
          initialFilled={false}
          onClose={onClose}
          onToggled={onToggled}
        />
      </TestQueryProvider>,
    );
    expect(
      screen.getByRole("heading", { name: /create link/i }),
    ).toBeInTheDocument();
    const createButton = await screen.findByRole("button", {
      name: /^create$/i,
    });
    await waitFor(() => expect(createButton).not.toBeDisabled());
    await userEvent.click(createButton);
    await waitFor(() => expect(putBodies.length).toBe(1));
    const body = putBodies[0] as {
      links?: Array<{ targetUuid?: string; type?: string }>;
    };
    expect(body.links).toEqual([
      {
        targetUuid: "other",
        type: "verifies",
        hint: {
          projectSlug: "sample",
          collectionPrefix: "REQ",
          artifactName: "REQ-other",
        },
      },
      {
        targetUuid: "c1",
        type: "satisfies",
        hint: {
          projectSlug: "sample",
          collectionPrefix: "DES",
          artifactName: "DES-a",
        },
      },
    ]);
    expect(onToggled).toHaveBeenCalledWith("created");
    expect(onClose).toHaveBeenCalled();
  });

  it("removes an existing link by PUTing the filtered list", async () => {
    const putBodies: unknown[] = [];
    vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const stripped = url.split("?")[0] ?? url;
      if (stripped.endsWith("/api/artifacts/r1") && init?.method === "PUT") {
        putBodies.push(init.body ? JSON.parse(String(init.body)) : undefined);
        return new Response("{}", {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      if (stripped.endsWith("/api/artifacts/r1")) {
        return new Response(
          JSON.stringify(
            artifactDetail("r1", "REQ-a", [
              {
                targetUuid: "c1",
                type: "satisfies",
                hint: {
                  projectSlug: "sample",
                  collectionPrefix: "DES",
                  artifactName: "DES-a",
                },
              },
              {
                targetUuid: "other",
                type: "verifies",
                hint: {
                  projectSlug: "sample",
                  collectionPrefix: "REQ",
                  artifactName: "REQ-other",
                },
              },
            ]),
          ),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      return new Response("{}", {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    });
    const onToggled = vi.fn();
    render(
      <TestQueryProvider>
        <MatrixCellDialog
          row={row}
          column={column}
          linkType="satisfies"
          initialFilled={true}
          onClose={() => {}}
          onToggled={onToggled}
        />
      </TestQueryProvider>,
    );
    expect(
      screen.getByRole("heading", { name: /remove link/i }),
    ).toBeInTheDocument();
    const removeButton = await screen.findByRole("button", {
      name: /^remove$/i,
    });
    await waitFor(() => expect(removeButton).not.toBeDisabled());
    await userEvent.click(removeButton);
    await waitFor(() => expect(putBodies.length).toBe(1));
    const body = putBodies[0] as {
      links?: Array<{ targetUuid?: string; type?: string }>;
    };
    // The satisfies link to c1 is gone; the verifies link stays.
    expect(body.links).toEqual([
      {
        targetUuid: "other",
        type: "verifies",
        hint: {
          projectSlug: "sample",
          collectionPrefix: "REQ",
          artifactName: "REQ-other",
        },
      },
    ]);
    expect(onToggled).toHaveBeenCalledWith("removed");
  });
});
