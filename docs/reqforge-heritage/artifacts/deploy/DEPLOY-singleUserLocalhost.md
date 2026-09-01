---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-deca5fe284c6",
  "title": "Single-user localhost posture (v1)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: IMkOrWdPxXOqXedfp-8BgVaFT_647wf1UNwoSS_C0k0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.19",
  "legacy": {
    "doorstopUid": "DEPLOY-singleUserLocalhost"
  }
}
---
The initial version of ReqForge shall assume a single user
accessing the web UI from the same host that runs the container
(or the host running a native dev build). ReqForge shall not
require authentication credentials and shall not enforce
authorization distinctions between hypothetical concurrent
users. This keeps the v1 surface small and matches the target
audience of personal projects and small-team use. Multi-user
authentication and authorization are explicitly deferred and
shall be revisited only when a concrete need drives them.
