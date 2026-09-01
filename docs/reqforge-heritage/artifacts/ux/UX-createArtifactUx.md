---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e5fdbb7a2927",
  "title": "Create-artifact UI split by shape",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: Cvjm-URQ5PMGp6m72AkX-kCQc4B-JiMlIRgeTE_Se_8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.7",
  "legacy": {
    "doorstopUid": "UX-createArtifactUx"
  }
}
---
Creating a new artifact from the UI shall use a different affordance
per artifact shape:
  - Content-hosted (text-like) artifacts: an in-browser editor.
  - Uploaded-blob artifacts of binary or complex structured shape
    (Microsoft Office documents, PDFs, images, and similar): an
    upload dialog.
  - URL-reference artifacts: a URL-entry form.
