---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dde29ad6b01c",
  "title": "Container upgrade flow",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: YpRVAP-JzwPEK-WsRx2hkVdjgX4L6GWhwNe2GpO-3Z0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.21",
  "legacy": {
    "doorstopUid": "DEPLOY-containerUpgrade"
  }
}
---
Upgrading a deployed ReqForge instance to a new release shall
be a two-command operation from the operator workspace (per
DEPLOY-operatorWorkspace):
  - docker compose pull (or equivalent) to fetch the new
    image.
  - docker compose up -d (or equivalent) to restart with the
    new image.
On restart, ReqForge rebuilds its in-memory UUID and search
indexes (per TRACE-uuidIndex and UX-search) and re-classifies
each mounted repository (per DEPLOY-mountValidityStates). The
schema-evolution behaviour of STOR-schemaForwardMigration and
STOR-schemaLazyWriteBack handles any file at an older schema
version; any file above the current schema loads its
containing Project read-only with the banner described by
STOR-schemaNewerFilesRefused. No dedicated upgrade migration
step, dedicated script, or ReqForge-initiated write is
required beyond the normal startup work; the upgrade recipe
shall be documented in the README.
