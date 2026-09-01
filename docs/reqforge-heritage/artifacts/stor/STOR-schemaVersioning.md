---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e410abb3a378",
  "title": "Schema versioning",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 3P_4xm-BcM0WwTBcLV0BweYpAtJ5XEhuJ_kn62KNaOw="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.3",
  "legacy": {
    "doorstopUid": "STOR-schemaVersioning"
  }
}
---
ReqForge shall embed a schema version in every file it writes —
project configuration files, per-Collection configuration files,
the System configuration file, and each artifact file — so the
on-disk format can evolve over time without breaking existing
projects. The detailed schema-migration design is captured in the
subsequent STOR-schema* items.
