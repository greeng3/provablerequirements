---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e64edad341fc",
  "title": "Project initialisation wizard",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: o8_jp1rskOiEVnP8aanJmGLwTUE-YqxBd94i6ry16To="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.19",
  "legacy": {
    "doorstopUid": "UX-initProjectWizard"
  }
}
---
The Initialize-as-ReqForge-project action (per UX-firstMountInit)
shall open a short wizard requesting three fields from the user:
slug (required; defaulted to the mount's directory name), name
(required; defaulted to the same), and description (optional).
On confirmation the wizard shall write reqforge.json at the
repository root, create the empty artifacts/ Collections
directory (or the path the user specified via artifactsPath if
they chose to override the default), and hand the user off to
the post-initialisation choice (per UX-postInitChoice).
