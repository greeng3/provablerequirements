---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b648c8fb574a",
  "title": "Optional outline level for document-heritage artifacts",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 440yoicnY8xFEqhE-E7Udo1YiYDse7qddtJqGN1btRs="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.17",
  "legacy": {
    "doorstopUid": "ART-outlineLevelField"
  }
}
---
Every artifact may carry an optional outlineLevel field (string,
for example, "1.2.3"). The field preserves outline-position
information from traditional requirements-document workflows
(IEEE 830, MIL-STD-490, and similar) and from doorstop imports
where the source document used an outline hierarchy. ReqForge
shall not use the field for automatic ordering of artifacts in
Collection views; when present, the UI shall surface it
inconspicuously next to the artifact title. Users who do not
work in outline-numbered conventions may leave the field unset.
