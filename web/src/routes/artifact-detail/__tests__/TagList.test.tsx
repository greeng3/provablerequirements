import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TagList } from "../TagList";

describe("TagList", () => {
  it("renders nothing when there are no tags", () => {
    const { container } = render(<TagList tags={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders each tag as a chip entry in a labelled list", () => {
    render(<TagList tags={["alpha", "beta", "gamma"]} />);
    const list = screen.getByRole("list", { name: /tags/i });
    expect(list).toBeInTheDocument();
    const items = screen.getAllByRole("listitem");
    expect(items.map((li) => li.textContent)).toEqual([
      "alpha",
      "beta",
      "gamma",
    ]);
  });

  it("does not wrap tags in links (no implicit linking per ART-tagsField)", () => {
    render(<TagList tags={["one"]} />);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });
});
