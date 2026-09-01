---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e45c6b5d985d",
  "title": "REST JSON HTTP API",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: In8l8QAXRRyHViVwjVl1OVthhUJkrjag1AKsdk4nFSI="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.3",
  "legacy": {
    "doorstopUid": "TECH-restApi"
  }
}
---
The ReqForge back-end shall expose its functionality to the
front-end via a REST-style HTTP API carrying JSON request and
response bodies. Resource endpoints cover Projects, Collections,
artifacts, links, review-log entries, reports, and supporting
metadata; standard HTTP verbs (GET, POST, PUT, PATCH, DELETE)
and status codes apply. Detailed endpoint specification is
implementation-time work. The API is an internal contract
between ReqForge's own back-end and front-end in the initial
version and is allowed to evolve freely in step with the UI;
it is not marketed as a stable external integration surface.
