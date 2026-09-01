---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e31a6cb43188",
  "title": "Rejected with blocking TODOs",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: LuZvStJU8GUUyyg_BfbBqxdpzhFomNWgAqrScDlxgU4="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.3",
  "legacy": {
    "doorstopUid": "REVIEW-blockingTodos"
  }
}
---
A rejected review may include a list of TODO items that must be
resolved before the artifact can be re-approved. These TODOs are
blocking: the artifact cannot reach an approved state without every
listed TODO being resolved. Advisory or non-blocking TODOs are
deliberately out of scope; those belong in a general issue-tracking
system, which ReqForge does not aspire to be.
