---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-def628e4e307",
  "title": "System configuration file",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: kwFltTyaHTEP_1Ionrp-iRh0_9hXq7Ru4RRhV7wRG_I="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.12",
  "legacy": {
    "doorstopUid": "DEPLOY-systemConfigFile"
  }
}
---
A System shall be defined by a configuration file that names the
System and lists the expected project slugs belonging to it. On
startup, ReqForge shall compare the mounted projects' slugs against
the expected list and surface any missing-mount discrepancies in the
UI (for example, "expected project X not mounted — mount the
repository to resolve cross-repository links").
