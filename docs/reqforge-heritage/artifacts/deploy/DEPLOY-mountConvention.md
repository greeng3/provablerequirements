---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-de40c87ed999",
  "title": "Mount discovery convention",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:07:48.491691470Z",
  "links": [
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-de55935c8061",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-mountValidityStates"
      }
    },
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
      "explanation": "Imported from doorstop; original reviewed hash: idNjheX8AhQm00_JGNdb7-mg_gvaXH-gyYkiG5IysPs="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.9",
  "legacy": {
    "doorstopUid": "DEPLOY-mountConvention"
  }
}
---
ReqForge shall discover projects by scanning a well-known in-container
path prefix (for example, /repos) for first-level subdirectories that
contain a .git entry. The prefix shall be configurable via environment
variable. No per-mount configuration shall be required beyond placing
each repository under the prefix.
