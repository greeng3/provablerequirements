---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e031ba4d6000",
  "title": "URL-reference artifacts as a single JSON file",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: a8N8CtY7Nt4DU8SPj7YUBqceo-f3PFjzotdRGpdV7t0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.4",
  "legacy": {
    "doorstopUid": "FORMAT-urlArtifactFile"
  }
}
---
Each URL-reference artifact shall be stored as a single
.reqforge.json file containing the URL and the artifact's metadata
(UUID, title, shape, review log, links, schema version, timestamps,
and any shape-specific fields). There is no separate body file for
URL artifacts; the URL itself is the content.
