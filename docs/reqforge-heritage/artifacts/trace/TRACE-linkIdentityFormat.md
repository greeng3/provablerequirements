---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e537b1225031",
  "title": "Link payload format",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 7MKpkwKX4IbOH4t4ZYBTn2D1Z7b8E7P-MT7M3Zjwksc="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.4",
  "legacy": {
    "doorstopUid": "TRACE-linkIdentityFormat"
  }
}
---
Each link shall store its target as the target artifact's UUID,
accompanied by a best-effort human hint consisting of project slug,
collection prefix, and current name. The UUID is authoritative for
resolution; the hint exists for human readability and for generating
clear messages about unresolved targets.
