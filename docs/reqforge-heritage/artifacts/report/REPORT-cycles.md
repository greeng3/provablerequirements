---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e26d8155612c",
  "title": "Cycle detection report",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: cxi6cwM2oQJZaN7ScQFcm8-58mxLbQN2CM7fcTET_wQ="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.5",
  "legacy": {
    "doorstopUid": "REPORT-cycles"
  }
}
---
ReqForge shall detect and report cycles in link types that are
expected to be acyclic (for example, derives-from). Cycles in
acyclic link types usually indicate a traceability modelling error
and shall be surfaced for resolution.
