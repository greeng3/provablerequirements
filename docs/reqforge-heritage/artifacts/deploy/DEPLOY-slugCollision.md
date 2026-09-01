---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dede68834315",
  "title": "Duplicate slug detection",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: hU-FVA2s7CVwaFuuQS52oRdFkugK_9Kfj7qwk5CZKys="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.11",
  "legacy": {
    "doorstopUid": "DEPLOY-slugCollision"
  }
}
---
When two or more mounted projects declare the same slug in their
reqforge.json files, ReqForge shall surface the collision as an error
in the UI and decline to operate on either colliding project until
the user resolves the conflict by editing one of the slugs.
