---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0c99f0e815d",
  "title": "Original doorstop files left untouched",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: EoJjvOwBJirp3nzbzibi6R-VrRcfgexznAv16v985Zc="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.9",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopPreserveOriginal"
  }
}
---
The doorstop importer shall not modify, move, or delete the
original doorstop files (.doorstop.yml marker files and per-item
yaml files). Imported ReqForge files are new files placed under
the target Project's Collections root. After verifying the
imported content, the user is responsible for removing or
archiving the original doorstop files from the repository as
they see fit; ReqForge never does so on behalf of the user.
