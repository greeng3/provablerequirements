---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e3d9e3ac5d53",
  "title": "Forward migration on load",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: eHG27ZDum1iKzFpp_bCvihoxj5CzJzQbqIwFDjFoNr8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.7",
  "legacy": {
    "doorstopUid": "STOR-schemaForwardMigration"
  }
}
---
When ReqForge loads a file whose schemaVersion is below the
current version known for that file type, it shall apply
migration functions in sequence (v1 to v2, v2 to v3, and so on)
to produce the current in-memory representation. Each migration
function is a named, deterministic transformation registered for
a specific single-step version bump; multi-step upgrades are the
sequential composition of these single-step migrations.
Deterministic migrations ensure that re-reading the same on-disk
file produces identical in-memory output regardless of how many
times the migration runs.
