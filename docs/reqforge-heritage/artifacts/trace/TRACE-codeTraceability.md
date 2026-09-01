---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e4e942341a69",
  "title": "Code and test traceability (future)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: PIPQj8RW5IRgPSZc0Pu13RrAoE0JDxBl15icQVlJWNg="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.8",
  "legacy": {
    "doorstopUid": "TRACE-codeTraceability"
  }
}
---
ReqForge shall support typed traceability from source code and test
files to requirements via a scan-and-report mechanism, accommodating
multiple programming languages. The detailed design is captured in
the subsequent TRACE-code* items. Implementation is deferred and
will be written in Rust inside ReqForge's back-end (per
TECH-rustBackend). The existing scripts/traceability.py is a design
reference only, not code to be preserved, vendored, or shelled out
to from ReqForge.
