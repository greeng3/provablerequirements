# provreq end-to-end guide

This is the whole process, in order, for one person (or one assisting LLM) taking a
requirement from prose all the way to a mechanically-checked verdict. It is written to
be read on its own: you should not need provreq's source, its web UI, or any other
document to follow it. If you are an LLM helping someone formalize their repository and
you can run `provreq`, run `provreq guide` to get this text, then use the tool itself as
the oracle — every step below tells you what a good result looks like and what the tool
says when you get it wrong.

Every command takes the subject repository as a path (defaulting to the current
directory). Run them from the subject, or pass `--path /path/to/subject`.

---

## What provreq does, in one paragraph

provreq turns a natural-language requirement into a **PRL** claim (a small formal
requirement language), binds the symbols in that claim to real things in your code, and
then asks a verification engine whether the claim holds. It is deliberately honest: it
never fabricates a verdict, and a verdict is **never stronger than its weakest binding**.
If it cannot ground or cannot check something, it says so and parks the requirement
rather than guessing.

## The mental model: brain vs. executor

Two separable roles:

- **The brain** — provreq itself plus, optionally, a configured LLM. It reads prose,
  proposes PRL, runs the mechanical gate, renders read-backs, and records verdicts.
- **The executor** — the verification engines (Kani, Creusot, Prusti, TLC, MonPoly, a
  WebDriver) that actually decide a formalized, grounded claim. Engines are optional and
  per-category; a missing engine parks a requirement, it does not fail it.

You can do the entire formalization pipeline (through admission) with no engine installed
at all. Engines only enter at the final `verify` step.

## The pipeline

```
init → triage → draft (set|translate) → check(gate) → readback → ground → admit → writeback → verify
```

Each requirement moves through this once. You can stop at any point and resume later; the
draft persists in the subject's untracked companion tree.

Everything below has a **command-line form** and an exact **web-UI counterpart**
(`provreq serve`, then open the drafting page). The UI is the same actions with a human
gate around the outward-facing ones; this guide uses the CLI because it is scriptable and
an assisting LLM can read its output.

---

## Step 0 — Adopt the subject (`init`)

```
provreq init            # scaffold the companion tree next to the subject's requirements
```

provreq discovers the subject's Doorstop/Provreq requirements layout and scaffolds a
co-located companion tree that holds drafts, groundings, and verdicts. Nothing about the
subject's own files changes here. Add `--yes` to skip the confirmation, `--name` to
override the companion tree's name.

If the subject has a Doorstop tree you want to convert to a native Provreq project first,
see `migrate-doorstop`; to author a brand-new requirement, see `new`; to validate the
project loads cleanly, `check`.

## Step 1 — Triage the backlog (`triage`)

```
provreq triage                        # classify items (advisory) and print the list
provreq triage --repeat               # keep re-asking until the backlog converges
provreq triage --set REQ001 formalizable-now
```

Triage sorts requirement items into buckets (e.g. `formalizable-now`) so you know what is
worth formalizing. It is **advisory** — you can override any item with `--set ID BUCKET`.

Triage uses the configured LLM if there is one. A classifier may decline an item, and
model nondeterminism declines a different subset each pass, so a single pass rarely
reaches zero untriaged. `--repeat` re-asks the still-untriaged residue round after round
until nothing is left, a round places nothing new, or a safety ceiling of rounds is hit —
it never coerces a decline into a decision. Use `--reclassify` to re-run over
already-triaged items (this replaces their classifications; `--yes` skips the prompt).

## Step 2 — Draft the candidate PRL (`draft --set` or `draft --translate`)

A **draft** is the working formalization of one requirement. Open or resume it by id:

```
provreq draft REQ001                                  # show the draft (omit id to list all)
provreq draft REQ001 --set 'require { never bad_state() }'   # author the PRL by hand
provreq draft REQ001 --translate                      # let the configured LLM propose it
```

`--set` writes the candidate PRL you supply (and re-baselines it against the current item
prose). `--translate` asks the configured LLM to forward-translate the prose into a
candidate (needs an `llm:` provider — see *Configuring an LLM* below). Translate replaces
the stored candidate, so save hand edits first.

Write the PRL against the grammar in *PRL quick reference* at the end of this guide (the
web UI shows the same reference next to the editor). The single most common mistake: a
`require` **property is exactly one pattern**; you do not combine whole patterns with
`and`. Multiple obligations are multiple property lines.

## Step 3 — Run the mechanical gate (`draft --check`)

```
provreq draft REQ001 --check
```

The gate parses the candidate and name/type-checks it against the declared vocabulary and
the requirement's category. This is a fast, deterministic, no-engine check. A failure
tells you exactly what is wrong (a parse error with the offending line, an undeclared
symbol, a pattern that does not fit the category — e.g. a liveness `leads_to` under
`category: 1`, which is temporal-free). **Iterate here against the tool**: fix the PRL,
re-check, until it passes. This is the same loop that gets a hand-authored claim green.

## Step 4 — Confirm the read-back (`draft --readback`)

```
provreq draft REQ001 --readback        # requires a gate pass
```

The read-back is a deterministic, controlled-natural-language surfacing of what the formal
claim actually *means*. Read it and confirm it matches the intent of the prose. This is
your defense against a claim that parses and type-checks but says the wrong thing (for
instance, a vacuously-true claim). If the read-back is wrong, go back to Step 2.

## Step 5 — Ground the vocabulary (`draft --ground`, `--fidelity`, `--dry-run`)

Grounding binds each **vocabulary symbol** in the claim to a concrete **observable** in
the real world, then dry-runs the binding so you can confirm *"here is what your binding
resolves to — is that what you meant?"* before any engine is trusted.

```
provreq draft REQ001 --ground 'bad_state=compute_state'          # symbol = observable
provreq draft REQ001 --ground 'User=crate::model::User' --fidelity definitional
provreq draft REQ001 --dry-run                                    # resolve bindings, report grounded/parked
```

### What an observable is

- A **predicate** symbol (an event or state name declared in the claim's vocabulary)
  binds to a concrete anchor. **For category 1 (code), the observable is the name of a
  function that stands for the predicate** — resolved against the subject's real syntax
  tree, *not* a string to grep for. Give the function's name (path-qualified if needed),
  not its body and not a description.
- A **sort** (a type a quantified variable ranges over, e.g. `each u: User`) binds to a
  real **type**. A predicate binds to a function; a sort binds to a type; they are bound
  separately and are not interchangeable.

A symbol the claim does not declare is rejected. A quantified claim whose sort names no
real type is not grounded — nothing can range over a domain that is not known to be real,
so it parks exactly as an unbound predicate does.

### Fidelity — how much a binding can be trusted (the key semantics)

Fidelity records how strong a binding's evidence is, because **a verdict is never
stronger than its weakest binding.** Three values:

| Fidelity        | Meaning                                                        | Default for category |
| --------------- | ------------------------------------------------------------- | -------------------- |
| `definitional`  | True by construction — a static structural fact, or a model variable. No live observation needed. | 1 (code), 2a (model) |
| `observed`      | A runtime observation that can be wrong.                       | 2b (runtime)         |
| `probed`        | A flaky UI probe.                                              | 3 (UI)               |

You usually do not pass `--fidelity` at all: the default comes from the requirement's
category, and the defaults are the right answer for almost every binding. Override only
when a specific binding is weaker (or, rarely, stronger) than its category's default.

For a category-1 code binding where the symbol maps one-to-one to a concrete code
construct, `definitional` with the observable set to that code symbol is exactly right —
it is true by definition, and no live stream is needed.

### The dry-run loop

`--dry-run` resolves the category-1 bindings against the subject's real source right now
(resolutions are recomputed live every time, because code moves under a binding just as
prose moves under a draft) and reports whether the requirement **grounds** or stays
**parked**. If a binding does not resolve, the dry-run says which one and why; fix the
observable and re-run. Categories 2a/2b/3 carry the same binding schema but their dry-run
is deferred until their engines/telemetry are wired — a deferred grounding never fakes a
verdict and leaves the requirement `admitted-but-ungrounded` (parked), not failed.

If you are unsure of the exact observable format, this is the place to iterate against the
tool: bind, dry-run, read the resolution or the rejection, adjust. It will tell you.

## Step 6 — Admit the formalization (`draft --admit`)

```
provreq draft REQ001 --admit --reviewer "Your Name"
```

Admission records that a human confirmed the read-back. The reviewer name is recorded as
provenance (defaults to `$USER`). If the candidate was flagged for mandatory review (e.g.
vacuity), admission requires confirming that first; `--yes` skips the prompt for
scripting.

## Step 7 — Write provenance back to the subject (`draft --writeback`)

```
provreq draft REQ001 --writeback
```

This is the **only** draft action that touches the subject's own files: it writes the
admitted formalization's provenance onto the subject requirement item, closing the
traceability loop. Review and commit the working-tree change yourself. If the prose has
moved since admission, writeback refuses (a stale write-back); re-admit against the
current prose first.

## Step 8 — Produce the verdict (`verify`)

```
provreq verify REQ001                         # honest three-valued verdict + provenance
provreq verify REQ001 --draft-contracts       # stage #[logic]/#[pure] markers for review
provreq verify REQ001 --draft-semantic        # LLM-draft #[requires]/#[ensures], staged
provreq verify REQ001 --draft-semantic --repair   # verify+repair contracts against the prover
```

`verify` produces the three-valued verdict — **proven / not-determined / disproven** —
with provenance (what implements and verifies the requirement, in which environment). If
the category's engine is not installed, the verdict is honestly `not-determined`/parked,
never a fabricated pass.

The optional contract-drafting flags stage *uncommitted working-tree edits* for you to
review — `--draft-contracts` adds deductive markers onto opaque predicate functions;
`--draft-semantic` asks the LLM to draft `#[requires]`/`#[ensures]` clauses (an untrusted
proposal the verifier re-checks); `--repair` runs the engine and repairs the drafted
contracts on the prover's feedback over a bounded number of rounds. provreq never commits;
it proposes, you review the diff, the verifier decides.

Use `provreq engines` to see which engines are installed and therefore which requirements
are checkable, and `provreq install <tlc|kani> --yes` to provision the light-tier engines
natively. `provreq report` prints the full traceability report; `provreq status` shows the
coverage funnel.

---

## Configuring an LLM (optional)

Triage, `draft --translate`, and `verify --draft-semantic` use a configured LLM; the rest
of the pipeline does not. Configure one without the UI:

```
provreq set-llm --model qwen2.5-coder:14b --endpoint http://localhost:11434   # local Ollama
provreq set-llm --model gpt-4o-mini --provider openai-compatible --endpoint <url> --api-key <key>
provreq set-llm --model claude-... --provider anthropic --transport cli       # via local claude CLI
```

Family is `openai-compatible` (Ollama/LMStudio/OpenAI), `anthropic`, or `gemini`. This
writes the same untracked `.provreq/system.json` the UI persists to, so CLI and UI share
one provider. If a model call fails, provreq surfaces the provider's actual reason (auth /
connection / model-not-found), so a bad model id or endpoint tells you what to fix.

## The web UI (`serve`)

```
provreq serve                 # http://127.0.0.1:17869, single subject, loopback only
```

Every action above has a UI counterpart on the drafting page: author/translate the
candidate, run the gate, read the read-back, add groundings, admit, write back, and
verify. The UI also shows the PRL grammar reference next to the candidate editor. It is
single-operator by design — one subject, loopback, no auth.

## The categories

Each requirement has a category that decides its observable world and its engine:

| Category | World    | Grounds against            | Default fidelity | Example engines        |
| -------- | -------- | -------------------------- | ---------------- | ---------------------- |
| 1        | code     | the subject's syntax tree  | `definitional`   | Kani, Creusot, Prusti  |
| 2a       | model    | a TLA+ spec definition     | `definitional`   | TLC                    |
| 2b       | runtime  | a runtime event/telemetry  | `observed`       | MonPoly                |
| 3        | UI       | declared UI steps          | `probed`         | WebDriver              |

A liveness pattern (`leads_to`, `eventually`) needs a runtime or model category (2a/2b) —
category 1 is temporal-free and takes only state predicates under `always`/`never`.

---

## PRL quick reference

The formal requirement language, enough to author or repair a claim:

- `require { <property> <property> … }` — a list; **all** must hold. No commas, no `and`
  between properties. Each obligation is its own property line.
- A property is `[each <var>: <Sort> .] <pattern> [<scope>]`. The `: <Sort>` is required
  (`each q .` alone will not parse), and a property holds **exactly one pattern**.
- Patterns (pick one): `never E` · `always E` · `eventually E` · `can_reach E` ·
  `E leads_to E [within T]` · `E precedes E` · `E occurs at most K times`.
- Scope: `globally` (default) · `before E` · `after E` · `between E and E`.
- An expression `E` is a predicate/event application like `pred(a, b, 409)`, combined with
  `and` / `or` / `not` / parens. **These join predicates inside one pattern — they never
  combine whole patterns.** Two obligations are two property lines, not
  `(A leads_to B) and (C leads_to D)`.
- Liveness (`leads_to`, `eventually`) cannot be `category: 1`.

For the complete grammar and semantics see `docs/requirement-language.md` in the provreq
repository.

## When something goes wrong

The whole tool is built to be iterated against — treat its output as the oracle:

- **Gate rejects the PRL** — the error names the parse/type/category problem and the line.
  Fix and `--check` again.
- **Read-back reads wrong** — the claim says the wrong thing; revise the PRL (Step 2).
- **Dry-run parks the requirement** — a binding did not resolve; the report says which
  symbol and why. Fix the observable (for category 1, the function name) and `--dry-run`
  again.
- **Verdict is `not-determined`** — usually the category's engine is not installed
  (`provreq engines`), not a defect in your formalization.
- **A model call fails** — provreq prints the provider's real reason; fix the model id,
  endpoint, or key with `set-llm` (or in the UI's provider panel).

## Related documents

- `docs/requirement-language.md` — the complete PRL grammar and semantics.
- `docs/installing-and-running.md` — getting the binary, engines, per-environment setup.
- `docs/applying-to-existing-repos.md` — the design behind adoption and the companion tree.
