---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e48d5b850f5e",
  "title": "Code-trace coverage expectation",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: OhBPdFM_FbzgeQUGa7usy6QL-jtzxDbieQG0Aez_j8M="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.18",
  "legacy": {
    "doorstopUid": "TRACE-codeCoverageExpectation"
  }
}
---
Each Collection shall carry an expectsCodeTrace flag, defaulting
to true, that controls whether its artifacts are expected to have
corresponding implementation and verification references in
source code. Individual artifacts may override the Collection
default via a per-artifact expectsCodeTrace field. Reports and
coverage calculations shall exempt no-trace-expected artifacts
from "uncovered" counts, generalising doorstop's non-functional
requirement exemption into a first-class ReqForge concept.
