---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e63c0dafa045",
  "title": "Initialize as ReqForge project",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: cmtBo8psLevzVAOepRSBOcE7KQ2AxJRbZUc52zw3TKE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.6",
  "legacy": {
    "doorstopUid": "UX-firstMountInit"
  }
}
---
When a mounted repository has a .git entry but no reqforge.json, the
UI shall offer an "Initialize as ReqForge project" action. The action
shall prompt the user for a project slug, write reqforge.json to the
repository root, and make the project appear as a normal project in
the System.
