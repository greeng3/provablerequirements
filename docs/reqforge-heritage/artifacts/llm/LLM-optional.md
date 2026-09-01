---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1a708fb7716",
  "title": "Optional LLM integration with graceful absence",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: dg5bF1rmqwKBZCQ-D1H_KsJFshRAYRWseWR25PCSIL0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.1",
  "legacy": {
    "doorstopUid": "LLM-optional"
  }
}
---
LLM integration shall be entirely optional. When no LLM
provider is configured, LLM-dependent UI affordances are
hidden; core ReqForge operations (artifact CRUD, link CRUD,
plain rename, reports) proceed with no LLM involvement.
Configuring an LLM unlocks features such as LLM-assisted
rename (per LLM-renameWorkflow) and whichever additional uses
ReqForge grows to support; it is never a hard prerequisite for
any core feature.
