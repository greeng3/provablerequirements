---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e4b586b0b66c",
  "title": "Code and test files are scanned, not persisted as artifacts",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 6qxqTpSY04QGNJIdlJphNbtav6_Fo-vC4q8R8L03VXY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.13",
  "legacy": {
    "doorstopUid": "TRACE-codeScanNotArtifacts"
  }
}
---
ReqForge shall not create ReqForge artifacts for source or test
files found to reference requirements. The scanner shall instead
produce overlay data on demand — a per-session mapping of
artifacts to the source locations that reference them. The typed-
link graph shall comprise only first-class artifacts; code
references feed reports and coverage calculations but do not
populate the artifact store.
