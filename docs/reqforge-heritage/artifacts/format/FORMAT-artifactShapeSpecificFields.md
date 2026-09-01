---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df1b6866e19b",
  "title": "Shape-specific artifact metadata fields",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 6cXlYgBnpI4RhItZH5xdT8_UVV0w-Vcj_CgvZdWG9gs="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.14",
  "legacy": {
    "doorstopUid": "FORMAT-artifactShapeSpecificFields"
  }
}
---
In addition to the common fields defined in
FORMAT-artifactMetadataSchema, artifact metadata shall carry
shape-specific fields:
  - Content-hosted artifacts: no additional metadata fields; the
    Markdown body after the frontmatter is the artifact content.
  - Blob artifacts: blobPath (string, required) — the
    repository-relative path to the paired binary file.
  - URL artifacts: url (string, required) — the external URL
    the artifact references; checkedAt (string, optional, UTC
    ISO 8601 timestamp) recording the most recent time the
    user triggered a URL check on this artifact; and
    checkStatus (string, optional) recording that check's
    outcome — for example "ok", "not-found", "server-error",
    "timeout", or a short free-form description. See
    UX-urlArtifactChecking for the triggering behaviour.
