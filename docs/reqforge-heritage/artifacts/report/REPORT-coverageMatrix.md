---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e257014b73dc",
  "title": "Coverage matrix report",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: XNN9xnGbMVAvheihR6eL547sibCIqym2N-z-QxrY0U0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.1",
  "legacy": {
    "doorstopUid": "REPORT-coverageMatrix"
  }
}
---
ReqForge shall produce a coverage matrix report that, for each
selected parent artifact, enumerates the child artifacts linked
to it via a configured set of "covering" traceability link
types. Parents with no covering children are flagged as gaps.
The default covering-link-type set shall be
{satisfies, verifies} — the classical interpretation that a
parent is covered when at least one design/implementation
claims to fulfil it and at least one verification (test,
manual-verification artifact, or similar) claims to confirm
it. The report accepts a configurable alternative set per
invocation; broader sets (including derives-from, cites, or
related-to) produce a looser notion of coverage, narrower
sets (for example, satisfies only) produce a stricter one.
The user's chosen set is persisted per saved report
configuration.
