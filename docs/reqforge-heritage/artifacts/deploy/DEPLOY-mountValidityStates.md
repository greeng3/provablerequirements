---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-de55935c8061",
  "title": "Mount validity handling",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:08:06.741132672Z",
  "links": [
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-dee8d07057ee",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-systemAboveProject"
      }
    }
  ],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: dRonEIdGN4gVPfA1dpxtP2Or2ca5lX5kZEu1QachiLE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.10",
  "legacy": {
    "doorstopUid": "DEPLOY-mountValidityStates"
  }
}
---
ReqForge shall classify each bind-mounted directory on discovery and
handle each state gracefully:
  - .git and reqforge.json both present: load as a project.
  - .git present, reqforge.json missing: surface as "not yet a ReqForge
    project" with an option to initialize it.
  - .git missing: show a warning banner and otherwise ignore the mount.
  - Mount is read-only: load read-only; disable write operations for
    that project and show a read-only banner in the UI.
