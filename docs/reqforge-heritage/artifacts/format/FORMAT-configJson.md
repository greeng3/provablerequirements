---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df53291ff656",
  "title": "Configuration files as JSON",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: tZTaHPQNirOwMXF1hNBV8tHdGcjh3kY1lq8tdLjXaiE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.5",
  "legacy": {
    "doorstopUid": "FORMAT-configJson"
  }
}
---
ReqForge's configuration files shall all be JSON:
  - Project configuration at a repository's root: reqforge.json.
  - Per-Collection configuration: .collection.json at each
    Collection's root directory.
  - System configuration: a JSON file whose path is determined at
    deployment time (bind-mounted or referenced via environment
    variable).
