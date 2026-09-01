---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e33399ad87dd",
  "title": "Review log",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 2l6ozpoVxWc-yj2kzqfDU07l4NC1X9l_uMvB1QeT1wo="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.1",
  "legacy": {
    "doorstopUid": "REVIEW-log"
  }
}
---
Each artifact shall carry a review log — a history of review events,
not a single flag. Each entry shall record reviewer identity,
timestamp, outcome, and any accompanying explanation. The log shall
be preserved as the artifact content evolves.
