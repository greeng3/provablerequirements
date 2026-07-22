import type { Classification, ItemState } from "../types";
import * as labels from "../labels";
import { Badge } from "./Badge";

const BUCKETS: { value: Classification; label: string }[] = [
  { value: "formalizable-now", label: "formalizable now" },
  { value: "falsifiable-only", label: "falsifiable only" },
  { value: "stays-prose", label: "stays prose" },
];

type Props = {
  items: ItemState[];
  onSelect: (id: string) => void;
  onTriage: (id: string, classification: Classification) => void;
};

export function RequirementsTable({ items, onSelect, onTriage }: Props) {
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
          const formal = labels.formalization(item.formalization);
          return (
            <tr
              key={item.id}
              onClick={() => onSelect(item.id)}
              className="cursor-pointer border-b border-border/60 align-top last:border-0 hover:bg-surface-2"
            >
              <td className="py-3 pr-4">
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onSelect(item.id);
                  }}
                  className="font-semibold tabular-nums hover:text-accent"
                >
                  {item.id}
                </button>
                <p className="mt-0.5 line-clamp-2 max-w-prose text-muted">
                  {item.title ?? item.text}
                </p>
              </td>
              <td className="py-3 pr-4">
                <TriageSelect item={item} onTriage={onTriage} />
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

type TriageSelectProps = {
  item: ItemState;
  onTriage: (id: string, classification: Classification) => void;
};

function TriageSelect({ item, onTriage }: TriageSelectProps) {
  return (
    <select
      aria-label={`Triage bucket for ${item.id}`}
      value={item.classification ?? ""}
      onClick={(e) => e.stopPropagation()}
      onChange={(e) => onTriage(item.id, e.target.value as Classification)}
      className="rounded-md border border-border bg-surface px-2 py-1 text-xs text-text hover:border-accent focus:border-accent focus:outline-none"
    >
      {item.classification === null && (
        <option value="" disabled>
          untriaged…
        </option>
      )}
      {BUCKETS.map((b) => (
        <option key={b.value} value={b.value}>
          {b.label}
        </option>
      ))}
    </select>
  );
}
