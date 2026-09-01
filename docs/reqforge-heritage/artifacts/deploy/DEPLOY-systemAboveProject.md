---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dee8d07057ee",
  "title": "System above Project",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:08:33.998016338Z",
  "links": [
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-deca5fe284c6",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-singleUserLocalhost"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-def628e4e307",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-systemConfigFile"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-deaad9dcd610",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-pollingWatch"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-deb445881cbc",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-projectConfigFile"
      }
    }
  ],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: KGH-cUvjP7qg1QkirnOq0LQu4t12Zg3QqaFX_P9K3jY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.4",
  "legacy": {
    "doorstopUid": "DEPLOY-systemAboveProject"
  }
}
---
ReqForge shall provide a System layer above Projects. A System is a
named collection of Projects whose artifacts can link to one
another. A ReqForge session operates within a System.
