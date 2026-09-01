---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df2fa728e517",
  "title": "Uploaded-blob artifacts as blob plus sidecar",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 2o7ZDvQoTrBj0okdAMjxwyIN3d3HiIgQp4sHZbrbMKY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.3",
  "legacy": {
    "doorstopUid": "FORMAT-blobSidecar"
  }
}
---
Each uploaded-blob artifact shall be stored as the original binary
file (retaining its native extension such as .pdf or .docx) paired
with a metadata sidecar named by appending .reqforge.json to the
binary's filename (for example, DES-spec.pdf and
DES-spec.pdf.reqforge.json). The sidecar holds the artifact
metadata (UUID, title, shape, review log, links, schema version,
timestamps, and any shape-specific fields).
