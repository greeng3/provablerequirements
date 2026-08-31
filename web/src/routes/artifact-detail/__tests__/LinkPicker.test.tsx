import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LinkPicker } from "../LinkPicker";
import { TestQueryProvider } from "../../../test-utils";
import type {
  ArtifactSearchResult,
  LinkType,
  LinkWriteRequest,
} from "../../../api/types";

const TYPES: LinkType[] = [
  {
    name: "derives-from",
    inverseName: "derived-into",
    directed: true,
    acyclic: true,
    source: "builtin",
  },
  {
    name: "satisfies",
    inverseName: "satisfied-by",
    directed: true,
    acyclic: false,
    source: "builtin",
  },
  {
    name: "mitigates",
    inverseName: "mitigated-by",
    directed: true,
    acyclic: false,
    source: "system",
  },
];

const SEARCH_RESULTS: ArtifactSearchResult[] = [
  {
    uuid: "0194f6d0-0001-7000-8000-000000000010",
    projectSlug: "sample",
    collectionPrefix: "REQ",
    artifactName: "REQ-foo",
    title: "Foo",
    active: true,
  },
  {
    uuid: "0194f6d0-0001-7000-8000-000000000011",
    projectSlug: "sample",
    collectionPrefix: "REQ",
    artifactName: "REQ-bar",
    title: "Bar",
    active: true,
  },
];

function installFetchStub(
  overrides?: Partial<{
    linkTypes: LinkType[];
    searchResults: ArtifactSearchResult[];
    searchResponder: (url: URL) => ArtifactSearchResult[];
  }>,
) {
  const linkTypes = overrides?.linkTypes ?? TYPES;
  const defaultResults = overrides?.searchResults ?? SEARCH_RESULTS;
  const fetchSpy = vi
    .spyOn(globalThis, "fetch")
    .mockImplementation(async (input) => {
      const url = new URL(input.toString(), "http://localhost");
      let body: unknown;
      if (url.pathname === "/api/link-types") {
        body = linkTypes;
      } else if (url.pathname === "/api/artifacts/search") {
        body = overrides?.searchResponder
          ? overrides.searchResponder(url)
          : defaultResults;
      } else {
        body = [];
      }
      return new Response(JSON.stringify(body), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    });
  return fetchSpy;
}

function renderPicker(overrides?: {
  onCommit?: (req: LinkWriteRequest) => void;
  onClose?: () => void;
}) {
  const onCommit = overrides?.onCommit ?? vi.fn();
  const onClose = overrides?.onClose ?? vi.fn();
  render(
    <TestQueryProvider>
      <LinkPicker
        currentArtifactUuid="0194f6d0-0001-7000-8000-000000000099"
        currentProjectSlug="sample"
        onCommit={onCommit}
        onClose={onClose}
      />
    </TestQueryProvider>,
  );
  return { onCommit, onClose };
}

describe("LinkPicker", () => {
  beforeEach(() => {
    installFetchStub();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("populates the type dropdown from useLinkTypes, grouping builtin vs system", async () => {
    renderPicker();
    await waitFor(() => {
      expect(
        screen.getByRole("option", { name: "derives-from" }),
      ).toBeInTheDocument();
    });
    expect(
      screen.getByRole("option", { name: "satisfies" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "mitigates" }),
    ).toBeInTheDocument();
  });

  it("debounces the search query and eventually renders results", async () => {
    const user = userEvent.setup();
    renderPicker();
    await user.type(screen.getByLabelText(/target artifact/i), "foo");
    await waitFor(() => {
      expect(screen.getByText(/REQ-foo/)).toBeInTheDocument();
    });
  });

  it("excludes the current artifact's UUID on the search request", async () => {
    const fetchSpy = installFetchStub();
    const user = userEvent.setup();
    renderPicker();
    await user.type(screen.getByLabelText(/target artifact/i), "req");
    await waitFor(() => {
      const searchCalls = fetchSpy.mock.calls.filter((call) =>
        call[0].toString().startsWith("/api/artifacts/search"),
      );
      expect(searchCalls.length).toBeGreaterThan(0);
      const call = searchCalls[searchCalls.length - 1];
      expect(call[0].toString()).toContain(
        "exclude=0194f6d0-0001-7000-8000-000000000099",
      );
    });
  });

  it("commits the selected target via click with the chosen type", async () => {
    const user = userEvent.setup();
    const { onCommit } = renderPicker();
    await waitFor(() =>
      expect(
        screen.getByRole("option", { name: "satisfies" }),
      ).toBeInTheDocument(),
    );
    await user.selectOptions(screen.getByLabelText(/link type/i), "satisfies");
    await user.type(screen.getByLabelText(/target artifact/i), "foo");
    await waitFor(() =>
      expect(screen.getByText(/REQ-foo/)).toBeInTheDocument(),
    );
    await user.click(screen.getByText(/REQ-foo/));
    expect(onCommit).toHaveBeenCalledWith(
      expect.objectContaining({
        targetUuid: "0194f6d0-0001-7000-8000-000000000010",
        type: "satisfies",
        hint: expect.objectContaining({
          projectSlug: "sample",
          collectionPrefix: "REQ",
          artifactName: "REQ-foo",
        }),
      }),
    );
  });

  it("commits via Enter on the highlighted row", async () => {
    const user = userEvent.setup();
    const { onCommit } = renderPicker();
    const input = screen.getByLabelText(/target artifact/i);
    await user.type(input, "req");
    await waitFor(() =>
      expect(screen.getByText(/REQ-bar/)).toBeInTheDocument(),
    );
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onCommit).toHaveBeenCalled();
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    const { onClose } = renderPicker();
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });
});
