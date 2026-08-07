// The D8 rungs are not equals, and rendering them identically was how the browser could make the
// weakest evidence provreq deals in look like the strongest (#229). `proven` and `model-checked
// (bounded)` are claims over all executions or over a model; `not-falsified` is a monitor saying
// nothing violated the property in the trace it actually read. It gets its own tone AND says the
// limit outright, because tone alone is a convention the operator has to have learned.
//
// The label strings arrive from the wire exactly as the CLI prints them (`Basis::as_str`), so the
// two surfaces cannot drift on wording — only on emphasis, which is what this file owns.
const EMPIRICAL = "not-falsified";

export function BasisLabel({ basis }: { basis: string }) {
  if (basis !== EMPIRICAL) {
    return <span className="text-xs text-muted">{basis}</span>;
  }
  return (
    <span className="text-xs text-warn">
      {basis} — what ran, not what can run
    </span>
  );
}
