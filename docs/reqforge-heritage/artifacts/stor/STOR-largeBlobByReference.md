---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e3a572fce327",
  "title": "Large content stored by reference",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: hIbpcndbBRiStKfZRbmLM0FmhxDdAfDruK3_giWb6KU="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.5",
  "legacy": {
    "doorstopUid": "STOR-largeBlobByReference"
  }
}
---
Content too large to reasonably live in the git tree shall be
represented as a URL-reference artifact rather than stored in-tree.
This keeps the git repository within reasonable size bounds without
requiring git LFS.
