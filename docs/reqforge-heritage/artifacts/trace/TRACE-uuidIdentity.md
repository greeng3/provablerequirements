---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e58bfb383025",
  "title": "UUID as stable artifact identity",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: OS1d-l-ubJMgD2H2ZtndoP41z7rbRXxwM8LShpoXBLU="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.3",
  "legacy": {
    "doorstopUid": "TRACE-uuidIdentity"
  }
}
---
Every artifact shall carry a UUID, assigned at creation, that serves
as its stable identity. The UUID shall persist across renames, moves
between directories, and changes to the artifact's human-readable
title or collection prefix.
