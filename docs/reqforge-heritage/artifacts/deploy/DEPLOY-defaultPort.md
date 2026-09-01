---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-ddf2444014c8",
  "title": "Default web UI port",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:07:58.828822169Z",
  "links": [
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-de2aaca04f50",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-envVars"
      }
    }
  ],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: R-TGQaWdmsE0gYFv9WB0G_-NptzcqM8il9d6-zw18Fo="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.15",
  "legacy": {
    "doorstopUid": "DEPLOY-defaultPort"
  }
}
---
The ReqForge web UI shall bind to TCP port 36743 by default.
This port was chosen for its distance from the most common
developer-tool defaults (3000, 5000, 8080, 8000, 9000, and
similar), reducing collision risk on typical developer
workstations, and because it spells "FORGE" on a phone keypad,
making it memorable. The default may be overridden via the
REQFORGE_PORT environment variable (per DEPLOY-envVars).
