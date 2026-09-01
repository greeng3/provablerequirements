---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e40f916d9898",
  "title": "Newer-than-current files are refused",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 48Y-6KUq6dGNhU8Ufty2Qivv0TGZ9SdUU82K-cH_a50="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.10",
  "legacy": {
    "doorstopUid": "STOR-schemaNewerFilesRefused"
  }
}
---
If ReqForge encounters a file whose schemaVersion is greater than
the current version known to the running ReqForge, the containing
Project shall be loaded read-only and the UI shall show a banner
explaining that the Project was written by a newer ReqForge and
requires an upgrade of the tool. ReqForge shall not attempt to
interpret unknown fields or partially write such files; all write
operations for that Project shall be disabled until the user
upgrades ReqForge.
