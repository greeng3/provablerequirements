import type { ItemState } from "../types";
import * as labels from "../labels";
import { Badge } from "./Badge";

type Props = {
  items: ItemState[];
};

export function RequirementsTable({ items }: Props) {
  if (items.length === 0) {
    return (
      <p role="status" className="py-8 text-center text-muted">
        No requirements in this view.
      </p>
    );
  }

  return (
    <table className="w-full border-collapse text-sm">
      <thead>
        <tr className="border-b border-border text-left text-xs uppercase tracking-wide text-muted">
          <th className="py-2 pr-4 font-medium">Item</th>
          <th className="py-2 pr-4 font-medium">Triage</th>
          <th className="py-2 font-medium">Formalization</th>
        </tr>
      </thead>
      <tbody>
        {items.map((item) => {
          const triage = labels.triage(item.classification);
          const formal = labels.formalization(item.formalization);
          return (
            <tr
              key={item.id}
              className="border-b border-border/60 align-top last:border-0"
            >
              <td className="py-3 pr-4">
                <div className="font-semibold tabular-nums">{item.id}</div>
                <p className="mt-0.5 line-clamp-2 max-w-prose text-muted">
                  {item.title ?? item.text}
                </p>
              </td>
              <td className="py-3 pr-4">
                <Badge label={triage.label} tone={triage.tone} />
              </td>
              <td className="py-3">
                <Badge label={formal.label} tone={formal.tone} />
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
