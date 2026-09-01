---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e3e3ea463b5e",
  "title": "Per-file-type schema versioning",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: DozujCmJJAYMYmWvU6XMu-eiZs95TGmfWFKapQ5alI0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.6",
  "legacy": {
    "doorstopUid": "STOR-schemaGranularity"
  }
}
---
Schema versions shall be tracked per file type rather than as a
single global value. Artifact metadata, project configuration,
per-Collection configuration, and the System configuration each
carry their own monotonically increasing integer schemaVersion
field. This lets each schema evolve independently and avoids
bumping (for example) the artifact version every time the
project-configuration format changes.

Nested structures within a file — link entries inside an
artifact's links array, review log entries and their TODOs
inside an artifact's reviewLog array, and similar — do not carry
their own schemaVersion. They ride on the parent file's version;
any change to their shape bumps the parent file's schemaVersion.
