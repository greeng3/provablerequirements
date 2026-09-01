---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0778f99f0e0",
  "title": "One-way import from doorstop",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:07:06.688491214Z",
  "links": [
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-e0c99f0e815d",
      "type": "satisfies",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "INTEROP",
        "artifactName": "INTEROP-doorstopPreserveOriginal"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-e0bd1a37cfce",
      "type": "satisfies",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "INTEROP",
        "artifactName": "INTEROP-doorstopPrefixCollision"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-e0a4dd62bb58",
      "type": "satisfies",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "INTEROP",
        "artifactName": "INTEROP-doorstopLinkTranslation"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-e09b441016f5",
      "type": "satisfies",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "INTEROP",
        "artifactName": "INTEROP-doorstopItemMapping"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-e06ac2d59cb5",
      "type": "satisfies",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "INTEROP",
        "artifactName": "INTEROP-doorstopIdNormalization"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-e05418f8431b",
      "type": "satisfies",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "INTEROP",
        "artifactName": "INTEROP-doorstopDocumentMapping"
      }
    },
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-e04cd47a4ad4",
      "type": "satisfies",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "INTEROP",
        "artifactName": "INTEROP-doorstopDiscovery"
      }
    }
  ],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: piUE1WLJUiqJYssiCPtDI4xFTR1lii8LcCdOHQkvBCA="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.1",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopImport"
  }
}
---
ReqForge shall support one-time, one-way import of requirements
and related metadata from an existing doorstop project into a
ReqForge Project. Round-tripping between ReqForge and doorstop is
explicitly out of scope. The detailed mapping rules, including
document-to-Collection translation, field-by-field item mapping,
link-type assignment, identifier normalisation, and import-report
shape, are captured in the subsequent INTEROP-doorstop* items.
