---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e5910fbe5587",
  "title": "UUID-to-path index",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: p5HoQWYzuYB3UqaqPzbPkhcHtoPR_PQYUI2GFRJnKyw="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.5",
  "legacy": {
    "doorstopUid": "TRACE-uuidIndex"
  }
}
---
On each ReqForge session, and whenever the set of mounted projects
changes, ReqForge shall build an index mapping artifact UUIDs to
their current on-disk locations across all mounted projects. Link
resolution shall consult this index. The index shall be held
entirely in memory and rebuilt on startup (and on mount changes);
it shall not be persisted to disk, to avoid drift between a cached
index and the mounted working trees.
