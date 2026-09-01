---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e23536d29ad7",
  "title": "Code and test traceability report",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: GItiJg8-JAWS84cCwvDmRgEt1tHOcxOBkohKzr-SXKA="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.15",
  "legacy": {
    "doorstopUid": "REPORT-codeTraceability"
  }
}
---
ReqForge shall produce a code and test traceability report that,
for each artifact, lists the source and test file locations
referencing it via in-code requirement tags. The report shall:
  - Group locations by tag verb (Satisfies vs Verifies, for
    example).
  - Flag orphan tags — tags in source code that fail to resolve
    to any existing artifact — separately, typically indicating
    a rename or a typo.
  - Flag artifacts whose Collection or per-artifact setting
    expects code trace but for which no matching tag was found
    (uncovered artifacts).
The report's implementation depends on the TRACE code scan
subsystem and is subject to the same scope-selector as other
reports.
