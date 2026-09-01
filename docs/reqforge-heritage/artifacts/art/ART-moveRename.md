---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b63a51fc5786",
  "title": "Move and rename artifacts",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: aaCdti7ByY3GDE2-ZmBTFpEVAKmLfeIVj_ZqRSeH0qA="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.13",
  "legacy": {
    "doorstopUid": "ART-moveRename"
  }
}
---
ReqForge shall support moving an artifact between Collections and
renaming an artifact from the UI. Because UUID is the authoritative
identity, moves and renames shall never break incoming links. Human-
readable hints on those incoming links shall be refreshed lazily when
the link is next resolved or displayed.
