# ReqForge heritage

Frozen, verbatim design documents copied out of the ReqForge peer
repository before it was retired (issue #402, Phase 6 of the
reqforge-absorb epic). ReqForge was a prior attempt at improving on
Doorstop; provreq absorbed its code — see
[absorbing-reqforge.md](../absorbing-reqforge.md) for the arc — but
this design rationale lived only in ReqForge's own tree.

Nothing here is maintained. It is kept as the record of _why_ the
absorbed subsystems are shaped the way they are, not as current
documentation. For how any of this behaves in provreq today, read the
code and the top-level `docs/`.

Contents:

- `artifacts/` — ReqForge's self-hosted requirement/spec artifacts
  (`LLM-*`, `TRACE-*`, `FORMAT-*`, `STOR-*`, `REPORT-*`, …), by
  collection. Each is a ReqForge Markdown artifact with JSON
  frontmatter — the same on-disk shape provreq now reads.
- `mcp.md` — ReqForge's MCP usage notes (the MCP server itself is
  absorbed as `crates/provreq-mcp`).
- `ROADMAP.md`, `INTENTIONS.md` — the design narrative and intent
  behind the system.

This tree is excluded from the prose formatter and linter (see
`.prettierignore` and the `lint-md` target): it is a byte-for-byte
copy, and reflowing it would turn a primary source into our
restatement of it — the same reason `tests/fixtures/reqforge-subject`
is excluded.
