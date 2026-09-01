---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e214a33e0e6a",
  "title": "Baseline export formats",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: vzwyQ2qrwpNEFotJ419SK7zyGESrIwhRgmr6k0CoYao="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.10",
  "legacy": {
    "doorstopUid": "REPORT-baselineExports"
  }
}
---
ReqForge shall support exporting reports as downloadable files
in the following baseline formats:
  - HTML, with hyperlinked navigation between referenced
    artifacts.
  - CSV. Every report has a tabular CSV representation with a
    stable column schema per report kind: matrix-shaped
    reports (coverage matrix, and similar) export as matrix
    CSVs with artifacts along rows and columns; list-shaped
    reports (orphans, conflicts, review status, unresolved
    links, and similar) export as row-per-entry CSVs with
    defined columns. Graph-shaped reports with no natural
    flat encoding may decline CSV with a clear message
    pointing at JSON or HTML.
  - JSON, for programmatic consumers.
The baseline export always reflects the selected scope of the
report being exported.
