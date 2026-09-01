---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e4dbbd51f2f2",
  "title": "Code tags reference human IDs, not UUIDs",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: lXl0vnrDTznIZDy1GWqm1b9NNolNVx52SGGY478yrfM="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.16",
  "legacy": {
    "doorstopUid": "TRACE-codeTagHumanIds"
  }
}
---
Tags in source code shall reference target artifacts by their
(collection prefix, artifact name) pair rather than by UUID.
Human-readable identifiers keep source comments legible at the
cost of fragility under rename: when a referenced artifact is
renamed, tags in source code do not update automatically. The
orphan-tag portion of the code-traceability report shall surface
such breakages so the user can correct the source comments.
