/// A collapsible, in-product PRL syntax reference sitting next to the candidate
/// editor (#465). When the mechanical gate rejects a hand-authored candidate,
/// the author has the grammar to consult here instead of guessing (the human
/// counterpart to #463/#464's fix to the model's translate prompt).
///
/// Native <details> so it is keyboard-operable and screen-reader-announced with
/// no custom state or key handling. Kept deliberately short — the full grammar
/// lives in docs/requirement-language.md, and the gate itself is src/prl/*.
/// Source of truth for these rules: src/prl/parser.rs. If the grammar changes,
/// update it there and here (no shared literal — one side is Rust, one is TS).

function Code({ children }: { children: React.ReactNode }) {
  return (
    <code className="rounded bg-slate-100 px-1 py-0.5 text-[11px] dark:bg-slate-800">
      {children}
    </code>
  );
}

export function PrlSyntaxReference() {
  return (
    <details className="rounded-lg border border-slate-200 bg-slate-50/60 text-xs dark:border-slate-800 dark:bg-slate-900/40">
      <summary className="cursor-pointer select-none rounded-lg px-3 py-2 font-medium text-slate-600 hover:text-slate-900 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-500 dark:text-slate-300 dark:hover:text-slate-100">
        PRL syntax
      </summary>
      <div className="flex flex-col gap-2 border-t border-slate-200 px-3 py-3 leading-relaxed text-slate-600 dark:border-slate-800 dark:text-slate-300">
        <p>
          <Code>require {"{"} …properties… {"}"}</Code> holds a list of
          properties — <strong>all</strong> must hold. No commas, no{" "}
          <Code>and</Code> between them; each obligation is its own line.
        </p>
        <p>
          A property is{" "}
          <Code>{"[each <var>: <Sort> .] <pattern> [<scope>]"}</Code> —
          the <Code>: {"<Sort>"}</Code> is required (<Code>each q .</Code>{" "}
          alone will not parse), and it holds{" "}
          <strong>exactly one pattern per property</strong>.
        </p>
        <div>
          <p>Patterns (pick one):</p>
          <ul className="ml-4 mt-1 list-disc space-y-0.5">
            <li>
              <Code>never E</Code> · <Code>always E</Code> ·{" "}
              <Code>eventually E</Code> · <Code>can_reach E</Code>
            </li>
            <li>
              <Code>{"E leads_to E [within T]"}</Code> ·{" "}
              <Code>E precedes E</Code>
            </li>
            <li>
              <Code>E occurs at most K times</Code>
            </li>
          </ul>
        </div>
        <p>
          Scope: <Code>globally</Code> (default) · <Code>before E</Code> ·{" "}
          <Code>after E</Code> · <Code>between E and E</Code>.
        </p>
        <p>
          An expression <Code>E</Code> is a predicate/event application like{" "}
          <Code>pred(a, b, 409)</Code>, combined with{" "}
          <Code>and</Code> / <Code>or</Code> / <Code>not</Code> / parens.{" "}
          <strong>
            These join predicates inside one pattern — they never combine whole
            patterns.
          </strong>{" "}
          Two obligations are two property lines, not{" "}
          <Code>(A leads_to B) and (C leads_to D)</Code>.
        </p>
        <p>
          Liveness (<Code>leads_to</Code>, <Code>eventually</Code>) needs a
          runtime/model category (2a or 2b) — <Code>category: 1</Code> (code) is
          temporal-free, so it takes only <Code>always</Code>/<Code>never</Code>{" "}
          over state predicates.
        </p>
      </div>
    </details>
  );
}
