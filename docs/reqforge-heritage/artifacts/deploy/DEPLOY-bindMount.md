---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b6e5da51e037",
  "title": "Bind-mounted git repositories",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: Tft8zitXmQMSEJNq3irt0doXimwwiGPsmoO4f9iOTd8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.2",
  "legacy": {
    "doorstopUid": "DEPLOY-bindMount"
  }
}
---
The ReqForge container shall consume git repositories via bind
mounts. ReqForge shall not clone, check out, or otherwise fetch
repositories on its own; the user is responsible for making the
repositories available to the container.
