---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e3b03cef4978",
  "title": "Bulk migrate action",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: RCe8p92HyPmovAZk7_U716nIfCDO8kRRu34j047TUhk="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.9",
  "legacy": {
    "doorstopUid": "STOR-schemaBulkMigrate"
  }
}
---
The ReqForge UI shall provide a per-Project "Migrate this Project
to the latest schema" action that rewrites every ReqForge-authored
file in the Project at the current schema version in a single
pass. Before performing the bulk rewrite, ReqForge shall detect
whether the Project's git working tree has uncommitted changes,
and if so, warn the user that the migration is best run from a
clean working tree so that the migration can be recorded as its
own commit. The warning shall offer confirm and cancel options;
ReqForge shall not itself commit the result.
