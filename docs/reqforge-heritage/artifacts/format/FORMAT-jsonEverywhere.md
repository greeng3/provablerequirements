---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dfb50d90baad",
  "title": "JSON for all structured data",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: jAipPXNnanSBtefFd8nlxqb9H75vtJ33uczNLz7hoq8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.1",
  "legacy": {
    "doorstopUid": "FORMAT-jsonEverywhere"
  }
}
---
JSON shall be the sole structured-data format used by ReqForge for
artifact metadata, configuration files, and frontmatter blocks.
YAML and TOML are explicitly not used. JSON is chosen for parse
performance, unambiguous syntax, and uniform tooling.
