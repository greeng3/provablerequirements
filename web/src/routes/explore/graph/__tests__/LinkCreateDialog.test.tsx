import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TestQueryProvider } from "../../../../test-utils";
import type { GraphNodeDto } from "../../../../api/types";
import { LinkCreateDialog } from "../LinkCreateDialog";

const source: GraphNodeDto = {
  uuid: "a",
  projectSlug: "sample",
  collectionPrefix: "REQ",
  artifactName: "REQ-a",
  title: "A",
  shape: "content",
  active: true,
  derived: false,
  tags: [],
};

const target: GraphNodeDto = {
  uuid: "b",
  projectSlug: "sample",
  collectionPrefix: "REQ",
  artifactName: "REQ-b",
  title: "B",
  shape: "content",
  active: true,
  derived: false,
  tags: [],
};

describe("LinkCreateDialog", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("PUTs the existing links plus the new entry when the user confirms", async () => {
    const putBodies: unknown[] = [];
    vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const stripped = url.split("?")[0] ?? url;
      if (stripped.endsWith("/api/link-types")) {
        return new Response(
          JSON.stringify([
            {
              name: "derives-from",
              inverseName: "derived-into",
              directed: true,
              acyclic: true,
              source: "builtin",
            },
            {
              name: "related-to",
              inverseName: "related-to",
              directed: false,
              acyclic: false,
              source: "builtin",
            },
          ]),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      if (stripped.endsWith("/api/artifacts/a") && init?.method === "PUT") {
        putBodies.push(init.body ? JSON.parse(String(init.body)) : undefined);
        return new Response("{}", {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      if (stripped.endsWith("/api/artifacts/a")) {
        return new Response(
          JSON.stringify({
            name: "REQ-a",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            uuid: "a",
            title: "A",
            shape: "content",
            description: null,
            active: true,
            derived: false,
            createdAt: "2026-04-22T00:00:00Z",
            modifiedAt: "2026-04-22T00:00:00Z",
            tags: [],
            links: [
              {
                targetUuid: "c",
                type: "verifies",
                hint: {
                  projectSlug: "sample",
                  collectionPrefix: "REQ",
                  artifactName: "REQ-c",
                },
                resolution: { state: "resolved" },
              },
            ],
            reviewLog: [],
            reviewState: { state: "none" },
            body: "",
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      return new Response("{}", {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    });

    const onCreated = vi.fn();
    const onClose = vi.fn();
    render(
      <TestQueryProvider>
        <LinkCreateDialog
          source={source}
          target={target}
          onClose={onClose}
          onCreated={onCreated}
        />
      </TestQueryProvider>,
    );

    // Default-selected link type is the alphabetically-first
    // catalog entry ("derives-from"). Wait for useArtifact to
    // populate so the Create button is enabled.
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
        targetUuid: "c",
        type: "verifies",
        hint: {
          projectSlug: "sample",
          collectionPrefix: "REQ",
          artifactName: "REQ-c",
        },
      },
      {
        targetUuid: "b",
        type: "derives-from",
        hint: {
          projectSlug: "sample",
          collectionPrefix: "REQ",
          artifactName: "REQ-b",
        },
      },
    ]);
    expect(onCreated).toHaveBeenCalledWith("derives-from");
    expect(onClose).toHaveBeenCalled();
  });

  it("blocks submission when the same link already exists", async () => {
    vi.stubGlobal("fetch", async (input: RequestInfo) => {
      const url = typeof input === "string" ? input : input.toString();
      const stripped = url.split("?")[0] ?? url;
      if (stripped.endsWith("/api/link-types")) {
        return new Response(
          JSON.stringify([
            {
              name: "derives-from",
              inverseName: "derived-into",
              directed: true,
              acyclic: true,
              source: "builtin",
            },
          ]),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      if (stripped.endsWith("/api/artifacts/a")) {
        return new Response(
          JSON.stringify({
            name: "REQ-a",
            projectSlug: "sample",
            collectionPrefix: "REQ",
            uuid: "a",
            title: "A",
            shape: "content",
            description: null,
            active: true,
            derived: false,
            createdAt: "2026-04-22T00:00:00Z",
            modifiedAt: "2026-04-22T00:00:00Z",
            tags: [],
            links: [
              {
                targetUuid: "b",
                type: "derives-from",
                hint: {
                  projectSlug: "sample",
                  collectionPrefix: "REQ",
                  artifactName: "REQ-b",
                },
                resolution: { state: "resolved" },
              },
            ],
            reviewLog: [],
            reviewState: { state: "none" },
            body: "",
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      return new Response("{}", {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    });
    render(
      <TestQueryProvider>
        <LinkCreateDialog
          source={source}
          target={target}
          onClose={() => {}}
          onCreated={() => {}}
        />
      </TestQueryProvider>,
    );
    // Once the source artifact's existing links load, the
    // duplicate guard should disable Create and surface a note.
    const createButton = await screen.findByRole("button", {
      name: /^create$/i,
    });
    await waitFor(() => expect(createButton).toBeDisabled());
    expect(screen.getByRole("alert")).toHaveTextContent(/already exists/i);
  });
});
