---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e570931e5234",
  "title": "Unresolved link reporting",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: c8qXgmRGm_X0nkjDIioeZyEVkrHMWc0Q_VPJwIM_JtA="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.7",
  "legacy": {
    "doorstopUid": "TRACE-unresolvedLinks"
  }
}
---
When a link's target UUID cannot be resolved against any currently
mounted project, ReqForge shall display the link as unresolved. The
unresolved state shall surface the human hint and shall identify the
project or repository that must be mounted to resolve it (for
example, "unresolved — mount repo X").
