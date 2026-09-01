---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1db3fcf1724",
  "title": "Generic prompt-response abstraction",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 5mYSZ7sV0gv3ETgj2Q0eWRUYcoLzz2mie70xDGt8ZIU="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.8",
  "legacy": {
    "doorstopUid": "LLM-promptAbstraction"
  }
}
---
The LLM provider adapter shall expose a generic "send prompt,
get response" interface rather than feature-specific methods.
Feature-level code (rename suggestion, future summarisation,
future link suggestion, extraction flows, and so on)
constructs its own prompt and parses the adapter's raw
response. This keeps the adapter layer minimal, allows new
LLM-dependent features to be added without changes to the
adapter layer, and concentrates prompt-engineering logic in
the feature code where context is available.
