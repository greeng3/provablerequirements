---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e381c2267b7e",
  "title": "One file per artifact",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: MTiDVoPtJTdYmSrjVtnXMeiwlP2WLhUj1VHP3KYRRfg="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.2",
  "legacy": {
    "doorstopUid": "STOR-filePerArtifact"
  }
}
---
Each managed artifact shall be stored as a discrete file. One file
per artifact keeps diffs small, makes reviews meaningful, and keeps
history clean across branches and merges.
