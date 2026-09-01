---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e08ff5e430e4",
  "title": "Post-import summary report",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: ppDuJp2HkTTdT4EdhYabJX8dCxPWTFdbd61DRXn0JX8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.11",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopImportReport"
  }
}
---
After a doorstop import run, ReqForge shall present a summary
report covering at minimum:
  - The number of Collections created and their prefixes.
  - The number of artifacts imported per Collection.
  - The number of typed links translated (all as derives-from).
  - The number and disposition of items whose ref field was
    non-null: URL refs that became URL artifacts plus cites
    links, and non-URL refs preserved in legacy.
  - The number of items whose custom fields were preserved in
    the legacy object.
  - The count of items with synthetic initial approved review
    entries from a doorstop reviewed hash.
  - Unresolved-link count, if any, flagged for follow-up.
  - Any warnings or errors encountered during the run.
The report shall be viewable in the UI and downloadable as a
file in one of the baseline report formats (per
REPORT-baselineExports).
