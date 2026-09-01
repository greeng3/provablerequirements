---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dfdab9dc4352",
  "title": "Links stored inline in artifact metadata",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 7gNaw1Y0WEcva9Ju6BC_lOf8mdr3SHEsMHfdG6nAumE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.7",
  "legacy": {
    "doorstopUid": "FORMAT-linksInlineInArtifact"
  }
}
---
Typed traceability links shall be stored as a field within the
owning artifact's metadata (JSON frontmatter for content-hosted
artifacts, sidecar file for blob artifacts, the single JSON file
for URL artifacts). Links are not stored in separate files. Each
link is an object containing at minimum the target UUID, the link
type, and the best-effort human hint (project slug, collection
prefix, current name).
