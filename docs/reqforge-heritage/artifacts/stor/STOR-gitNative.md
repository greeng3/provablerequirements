---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e393676c1a2a",
  "title": "Git-native storage",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: gpXBrVVRMqZptCZiJqhLFgejcRhk6lQSmGzVyjJp77w="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.1",
  "legacy": {
    "doorstopUid": "STOR-gitNative"
  }
}
---
ReqForge shall store all managed artifacts as plain files inside the
user's git repositories. Artifacts shall be versioned, branched,
diffed, reviewed, and merged using the same git workflows that apply
to source code. ReqForge shall not maintain a separate database of
record outside those repositories.
