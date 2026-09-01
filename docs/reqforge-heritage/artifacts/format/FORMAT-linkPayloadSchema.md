---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dfc4ae8615e0",
  "title": "Link payload schema",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: xpm-TOr5D2mvsZE6p8bum_q9WIGPQKDUEek1AUsLHV4="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.15",
  "legacy": {
    "doorstopUid": "FORMAT-linkPayloadSchema"
  }
}
---
Each entry in an artifact's links array shall have:
  - targetUuid (string, required): the target artifact's UUID.
  - type (string, required): the link-type name, which must
    match either a built-in link type (per TRACE-linkCatalog) or
    a System-declared type (per TRACE-linkExtensibility).
  - hint (object, required): best-effort human-readable pointer
    with three string fields — projectSlug, collectionPrefix,
    and artifactName.
