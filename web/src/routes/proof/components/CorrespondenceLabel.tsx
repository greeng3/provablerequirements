// Mechanical evidence is a provreq-generated harness lowered from the formal
// claim, so a pass means the requirement holds; asserted evidence is a human's
// `Verifies:` claim that a tagged test checks it — provreq confirms the test
// runs and passes, never that passing it *means* the requirement holds. Drawing
// the two alike was how an asserted `holds` could read as strong as a
// mechanical one; the CLI prints `[mechanical]`/`[asserted]` on every verdict
// for exactly this reason (report.rs), and this surface must carry it too.
//
// Strings arrive from the wire as `Correspondence::as_str` prints them, so the
// two surfaces cannot drift on wording.
const ASSERTED = "asserted";

export function CorrespondenceLabel({
  correspondence,
}: {
  correspondence: string;
}) {
  if (correspondence !== ASSERTED) {
    return <span className="text-xs text-slate-400">[{correspondence}]</span>;
  }
  return (
    <span className="text-xs text-amber-700 dark:text-amber-300">
      [asserted] — a tagged test, not a proof
    </span>
  );
}
