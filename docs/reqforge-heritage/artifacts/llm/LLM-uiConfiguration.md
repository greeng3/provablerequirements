---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e30000000001",
  "title": "LLM provider configuration via the UI",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:13:47.861628511Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T23:13:47.861628511Z",
      "reviewer": "greeng3",
      "outcome": "approved"
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.5",
  "legacy": {
    "doorstopUid": "LLM-uiConfiguration"
  }
}
---
ReqForge shall expose Add / Edit / Delete / Enable / Reorder
controls for LLM provider entries on the `/llm` page. Operators
shall not be required to hand-edit the System configuration
file or restart the server to add or change a provider; CRUD
through the UI shall persist updates by atomic-writing the
System config file in place. The form shall accept the apiKey
inline (no environment-variable indirection per LLM-secretsViaEnv).
The Enable toggle shall flip the provider's `enabled` flag (per
LLM-configSchema). Reorder shall change the provider's index
(priority) in the array.

The endpoints implementing this surface (`POST` /
`PUT` / `DELETE` / `PATCH` under `/api/llm/providers`) shall
require write access to the System config file; the loader
shall surface a clear error when the file is read-only or
missing.
