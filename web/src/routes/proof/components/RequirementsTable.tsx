import type {
  ProofClassification,
  ProofItemState,
  ProofOrigin,
} from "../../../api/types";
import { formalizationLabel, originNote } from "../labels";
import { Badge } from "./Badge";
import { VerdictBadge } from "./VerdictBadge";

const BUCKETS: { value: ProofClassification; label: string }[] = [
  { value: "formalizable-now", label: "formalizable now" },
  { value: "falsifiable-only", label: "falsifiable only" },
  { value: "stays-prose", label: "stays prose" },
];

type Props = {
  items: ProofItemState[];
  onSelect: (id: string) => void;
  onTriage: (id: string, classification: ProofClassification) => void;
};

export function RequirementsTable({ items, onSelect, onTriage }: Props) {
  if (items.length === 0) {
    return (
      <p role="status" className="py-8 text-center text-slate-500">
        No requirements in this view.
      </p>
    );
  }

  return (
    <table className="w-full border-collapse text-sm">
      <thead>
        <tr className="border-b border-slate-200 text-left text-xs uppercase tracking-wide text-slate-500 dark:border-slate-800">
          <th className="py-2 pr-4 font-medium">Item</th>
          <th className="py-2 pr-4 font-medium">Triage</th>
          <th className="py-2 pr-4 font-medium">Formalization</th>
          <th className="py-2 font-medium">Verdict</th>
        </tr>
      </thead>
      <tbody>
        {items.map((item) => {
          const formal = formalizationLabel(item.formalization);
          return (
            <tr
              key={item.id}
              onClick={() => onSelect(item.id)}
              className="cursor-pointer border-b border-slate-100 align-top last:border-0 hover:bg-slate-50 dark:border-slate-800/60 dark:hover:bg-slate-800/50"
            >
              <td className="py-3 pr-4">
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onSelect(item.id);
                  }}
                  className="font-semibold tabular-nums hover:text-sky-600 dark:hover:text-sky-400"
                >
                  {item.id}
                </button>
                <p className="mt-0.5 line-clamp-2 max-w-prose text-slate-500">
                  {item.title ?? item.text}
                </p>
              </td>
              <td className="py-3 pr-4">
                <TriageSelect item={item} onTriage={onTriage} />
                <OriginNote origin={item.classified_by} />
              </td>
              <td className="py-3 pr-4">
                <Badge label={formal.label} tone={formal.tone} />
              </td>
              <td className="py-3">
                {item.verdict ? (
                  <VerdictBadge verdict={item.verdict} />
                ) : (
                  <span className="text-xs text-slate-500">not verified</span>
                )}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

/// What produced this row's bucket, rendered only when that is worth less than
/// the bucket looks (#180). A `stays-prose` nothing judged means *this will not
/// be formalized*, reached because nothing could decide — and the backlog is the
/// one surface where a whole set of buckets is read at a glance.
function OriginNote({ origin }: { origin: ProofOrigin | null }) {
  const note = originNote(origin);
  if (!note) return null;
  return <p className="mt-1 text-xs italic text-slate-500">{note}</p>;
}

type TriageSelectProps = {
  item: ProofItemState;
  onTriage: (id: string, classification: ProofClassification) => void;
};

function TriageSelect({ item, onTriage }: TriageSelectProps) {
  return (
    <select
      aria-label={`Triage bucket for ${item.id}`}
      value={item.classification ?? ""}
      onClick={(e) => e.stopPropagation()}
      onChange={(e) =>
        onTriage(item.id, e.target.value as ProofClassification)
      }
      className="rounded-md border border-slate-300 bg-white px-2 py-1 text-xs text-slate-700 hover:border-sky-500 focus:border-sky-500 focus:outline-none dark:border-slate-600 dark:bg-slate-800 dark:text-slate-200"
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
