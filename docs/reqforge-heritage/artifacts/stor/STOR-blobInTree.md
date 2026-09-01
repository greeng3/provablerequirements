---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e37c364ac111",
  "title": "Blob artifacts stored in-tree",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 9FEX9bt18wwIcMn7uUjmjZUE1HxceJPWSLm5UZZrElo="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.4",
  "legacy": {
    "doorstopUid": "STOR-blobInTree"
  }
}
---
Binary artifacts of reasonable size (uploaded PDFs, Office documents,
images, and similar) shall be stored directly in the project's git
tree. Git LFS support is deliberately out of scope for the initial
version and may be added later if artifact sizes demand it.
