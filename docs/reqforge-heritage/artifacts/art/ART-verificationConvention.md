---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b6d069c048f4",
  "title": "Verifications as regular artifacts",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:10:22.258658138Z",
  "links": [
    {
      "targetUuid": "019df9d6-cbf5-7133-a8e2-b6cf2d833dfb",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "ART",
        "artifactName": "ART-useCases"
      }
    },
    {
      "targetUuid": "019df9d6-cbf5-7133-a8e2-b65d6a50e3c1",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "ART",
        "artifactName": "ART-requirements"
      }
    }
  ],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: _Tf7UsTsprA-OTOc_txgAH5XM5fLtbGe0e6KiOP06QE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.20",
  "legacy": {
    "doorstopUid": "ART-verificationConvention"
  }
}
---
ReqForge shall not introduce a first-class "verification"
concept distinct from regular artifacts. Verification
activities — whether automated test references, manual test
procedures, demo records, observation notes, or the existence
and approval of a deliverable document — are represented as
ordinary artifacts, typically gathered in a dedicated
Collection (for example, prefix VER). Each verification
artifact carries one or more "verifies" links to the
requirements it confirms. Coverage reports count incoming
"verifies" links regardless of whether the source artifact
references code, a manual procedure, or simply the existence
of an approved deliverable; the review workflow on the
verification artifact provides accountability. Requirements
that are themselves satisfied by a deliverable document
(for example, "the system shall have installation
documentation") may rely on the document's own approved
review state as sufficient verification, or may additionally
cite a VER artifact that records someone's independent
judgement that the document meets the requirement.
