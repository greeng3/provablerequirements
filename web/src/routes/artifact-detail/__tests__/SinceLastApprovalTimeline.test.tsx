import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SinceLastApprovalTimeline } from "../SinceLastApprovalTimeline";
import type { OpenTodo, ReviewLogEntry } from "../../../api/types";

const OPEN: OpenTodo = {
  id: "t1",
  text: "Still open",
  addedAt: "2026-04-19T00:00:00Z",
  addedBy: "bob",
};

const LOG: ReviewLogEntry[] = [
  {
    timestamp: "2026-04-18T00:00:00Z",
    reviewer: "alice",
    outcome: "approved",
  },
  {
    timestamp: "2026-04-19T00:00:00Z",
    reviewer: "bob",
    outcome: "rejected",
    addedTodos: [
      { id: "t1", text: "Still open" },
      { id: "t0", text: "Resolved long ago" },
    ],
  },
  {
    timestamp: "2026-04-19T01:00:00Z",
    reviewer: "carol",
    outcome: "todo-resolved",
    resolvedTodos: ["t0"],
  },
];

describe("SinceLastApprovalTimeline", () => {
  it("renders entries newer than the last approval only", () => {
    render(
      <SinceLastApprovalTimeline
        log={LOG}
        lastApprovalAt="2026-04-18T00:00:00Z"
        blockingTodos={[OPEN]}
      />,
    );
    // The approved entry itself is at lastApprovalAt, so it's
    // excluded; rejected and todo-resolved entries should render.
    expect(screen.getByText("rejected")).toBeInTheDocument();
    expect(screen.getByText("todo-resolved")).toBeInTheDocument();
    expect(screen.queryByText("approved")).not.toBeInTheDocument();
  });

  it("marks pending TODOs with the amber indicator", () => {
    render(
      <SinceLastApprovalTimeline
        log={LOG}
        lastApprovalAt="2026-04-18T00:00:00Z"
        blockingTodos={[OPEN]}
      />,
    );
    expect(screen.getByLabelText("pending TODO")).toBeInTheDocument();
    expect(screen.getByLabelText("resolved TODO")).toBeInTheDocument();
  });

  it("shows an empty-window message when nothing happened since approval", () => {
    render(
      <SinceLastApprovalTimeline
        log={[LOG[0]]}
        lastApprovalAt="2026-04-18T00:00:00Z"
        blockingTodos={[]}
      />,
    );
    expect(
      screen.getByText(/No activity since the last approval/),
    ).toBeInTheDocument();
  });
});
